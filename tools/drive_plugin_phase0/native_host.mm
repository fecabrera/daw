// BSD-2-Clause. A small macOS ABI host for disposable Phase 0 probes.
// All SDK pointers remain owned by their creating side. Plugin instances and
// editor views are destroyed before their libraries/bundles are released.
#import <Cocoa/Cocoa.h>
#include <dlfcn.h>
#include <clap/clap.h>
#include "pluginterfaces/base/ipluginbase.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include "pluginterfaces/vst/ivsthostapplication.h"
#include "pluginterfaces/vst/ivstparameterchanges.h"
#include "pluginterfaces/gui/iplugview.h"
#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstring>
#include <iostream>
#include <stdexcept>
#include <thread>
#include <vector>

using namespace Steinberg;
using namespace Steinberg::Vst;
static thread_local bool audio_thread = false;
static void require(bool ok, const char* message) {
    if (!ok) throw std::runtime_error(message);
}
static void pump(double seconds) {
    NSDate* until = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while ([until timeIntervalSinceNow] > 0) {
        NSEvent* event = [NSApp nextEventMatchingMask:NSEventMaskAny
            untilDate:[NSDate dateWithTimeIntervalSinceNow:0.005]
            inMode:NSDefaultRunLoopMode dequeue:YES];
        if (event) [NSApp sendEvent:event];
        [NSApp updateWindows];
    }
}
static NSWindow* window() {
    [NSApplication sharedApplication];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
    NSWindow* w = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 360, 120)
        styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
    [w setTitle:@"DAW Phase 0 feasibility probe"];
    [w setReleasedWhenClosed:NO];
    [w orderFront:nil];
    return w;
}

struct Samples {
    std::array<float, 2048> left{}, right{}, out_left{}, out_right{};
    float* in[2] = {left.data(), right.data()};
    float* out[2] = {out_left.data(), out_right.data()};
    Samples() { left.fill(0.5f); right.fill(-0.25f); }
    void check(uint32_t frames, uint32_t change, float gain) {
        for (uint32_t i = 0; i < frames; ++i) {
            float g = i < change ? 1.0f : gain;
            require(out_left[i] == left[i]*g && out_right[i] == right[i]*g,
                "host-owned stereo buffer or timed event mismatch");
        }
    }
};

static const void* CLAP_ABI clap_extension(const clap_host_t*, const char* id) {
    static const clap_host_thread_check_t threads = {
        [](const clap_host_t*) -> bool { return !audio_thread && [NSThread isMainThread]; },
        [](const clap_host_t*) -> bool { return audio_thread; }
    };
    static const clap_host_gui_t gui = {
        [](const clap_host_t*) {},
        [](const clap_host_t*, uint32_t, uint32_t) -> bool { return true; },
        [](const clap_host_t*) -> bool { return true; },
        [](const clap_host_t*) -> bool { return true; },
        [](const clap_host_t*, bool) {}
    };
    if (!strcmp(id, CLAP_EXT_THREAD_CHECK)) return &threads;
    if (!strcmp(id, CLAP_EXT_GUI)) return &gui;
    return nullptr;
}
static void CLAP_ABI clap_request(const clap_host_t*) {}
struct ClapEvents {
    std::vector<clap_event_param_value_t> events;
    clap_input_events_t input = {this,
        [](const clap_input_events_t* list) -> uint32_t {
            return static_cast<ClapEvents*>(list->ctx)->events.size();
        },
        [](const clap_input_events_t* list, uint32_t index) -> const clap_event_header_t* {
            const auto& events = static_cast<ClapEvents*>(list->ctx)->events;
            return index < events.size() ? &events[index].header : nullptr;
        }};
    void value(clap_id id, uint32_t time, double value) {
        clap_event_param_value_t e{};
        e.header = {sizeof(e), time, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_PARAM_VALUE, 0};
        e.param_id = id;
        e.note_id = e.port_index = e.channel = e.key = -1;
        e.value = value;
        events.push_back(e);
    }
};
static void run_clap(void* library, const char* bundle, bool gui) {
    auto* entry = static_cast<const clap_plugin_entry_t*>(dlsym(library, "clap_entry"));
    require(entry && entry->init(bundle), "CLAP entry/init");
    auto* factory = static_cast<const clap_plugin_factory_t*>(entry->get_factory(CLAP_PLUGIN_FACTORY_ID));
    require(factory && factory->get_plugin_count(factory) == 1, "CLAP discovery");
    const auto* descriptor = factory->get_plugin_descriptor(factory, 0);
    require(descriptor && descriptor->id, "CLAP descriptor");
    clap_host_t host{CLAP_VERSION, nullptr, "Phase 0 ABI host", "DAW", "", "0.0.0",
        clap_extension, clap_request, clap_request, clap_request};
    auto* plugin = factory->create_plugin(factory, &host, descriptor->id);
    require(plugin && plugin->init(plugin), "CLAP create/init instance");
    auto* parameters = static_cast<const clap_plugin_params_t*>(plugin->get_extension(plugin, CLAP_EXT_PARAMS));
    require(parameters && parameters->count(plugin) == 1, "CLAP parameter discovery");
    clap_param_info_t info{};
    require(parameters->get_info(plugin, 0, &info), "CLAP parameter metadata");
    require(plugin->activate(plugin, 48000, 1, 64), "CLAP activation");
    std::exception_ptr failure;
    std::thread processing([&] {
        audio_thread = true;
        try {
            require(plugin->start_processing(plugin), "CLAP start");
            Samples samples;
            clap_audio_buffer_t in{samples.in, nullptr, 2, 0, 0};
            clap_audio_buffer_t out{samples.out, nullptr, 2, 0, 0};
            ClapEvents events;
            clap_output_events_t output{nullptr, [](const clap_output_events_t*, const clap_event_header_t*) -> bool { return true; }};
            clap_process_t p{};
            p.frames_count = 64;
            p.audio_inputs = &in; p.audio_outputs = &out;
            p.audio_inputs_count = p.audio_outputs_count = 1;
            p.in_events = &events.input; p.out_events = &output;
            require(plugin->process(plugin, &p) != CLAP_PROCESS_ERROR, "CLAP process");
            samples.check(64, 64, 1.0f);
            events.value(info.id, 17, 0.25);
            p.steady_time = 64;
            require(plugin->process(plugin, &p) != CLAP_PROCESS_ERROR, "CLAP timed process");
            samples.check(64, 17, 0.25f);
            events.events.clear();
            p.steady_time += 64; p.frames_count = 1;
            require(plugin->process(plugin, &p) != CLAP_PROCESS_ERROR, "CLAP variable block");
            samples.check(1, 0, 0.25f);
            plugin->reset(plugin);
            plugin->stop_processing(plugin);
        } catch (...) { failure = std::current_exception(); }
    });
    processing.join();
    if (failure) std::rethrow_exception(failure);
    int cycles = 0;
    if (gui) {
        auto* editor = static_cast<const clap_plugin_gui_t*>(plugin->get_extension(plugin, CLAP_EXT_GUI));
        require(editor && editor->is_api_supported(plugin, CLAP_WINDOW_API_COCOA, false), "CLAP Cocoa editor");
        NSWindow* w = window();
        for (int i = 0; i < 3; ++i) {
            require(editor->create(plugin, CLAP_WINDOW_API_COCOA, false), "CLAP editor create");
            clap_window_t parent{}; parent.api = CLAP_WINDOW_API_COCOA;
            parent.cocoa = (__bridge void*)[w contentView];
            require(editor->set_parent(plugin, &parent) && editor->show(plugin), "CLAP editor embedding");
            pump(0.15);
            require([[[w contentView] subviews] count] > 0, "CLAP native child view missing");
            editor->hide(plugin); editor->destroy(plugin); pump(0.05);
            ++cycles;
        }
        [w close];
    }
    plugin->deactivate(plugin); plugin->destroy(plugin); entry->deinit();
    std::cout << "{\"format\":\"clap\",\"discovery\":true,\"stereo_buffers\":true,\"sample_offset_17\":true,\"variable_blocks\":true,\"editor_cycles\":" << cycles << "}\n";
}

// Host objects live until after plugin termination. Their initial reference is
// held by this stack frame; addRef/release never delete stack storage.
#define HOST_UNKNOWN(Interface) \
    std::atomic<uint32> refs{1}; \
    uint32 PLUGIN_API addRef() override { return ++refs; } \
    uint32 PLUGIN_API release() override { return --refs; } \
    tresult PLUGIN_API queryInterface(const TUID iid, void** result) override { \
        if (!result) return kInvalidArgument; *result = nullptr; \
        if (FUnknownPrivate::iidEqual(iid, Interface::iid.toTUID()) || \
            FUnknownPrivate::iidEqual(iid, FUnknown::iid.toTUID())) { \
            *result = static_cast<Interface*>(this); addRef(); return kResultOk; \
        } return kNoInterface; \
    }
struct VstHost : IHostApplication {
    HOST_UNKNOWN(IHostApplication)
    tresult PLUGIN_API getName(String128 name) override {
        const char16 text[] = u"Phase 0 ABI host";
        std::copy(std::begin(text), std::end(text), name); return kResultOk;
    }
    tresult PLUGIN_API createInstance(TUID, TUID, void** object) override {
        if (object) *object = nullptr; return kNoInterface;
    }
};
struct Queue : IParamValueQueue {
    HOST_UNKNOWN(IParamValueQueue)
    ParamID id = 0;
    std::vector<std::pair<int32, ParamValue>> points;
    ParamID PLUGIN_API getParameterId() override { return id; }
    int32 PLUGIN_API getPointCount() override { return points.size(); }
    tresult PLUGIN_API getPoint(int32 index, int32& offset, ParamValue& value) override {
        if (index < 0 || size_t(index) >= points.size()) return kInvalidArgument;
        offset = points[index].first; value = points[index].second; return kResultOk;
    }
    tresult PLUGIN_API addPoint(int32 offset, ParamValue value, int32& index) override {
        index = points.size(); points.emplace_back(offset, value); return kResultOk;
    }
};
struct Changes : IParameterChanges {
    HOST_UNKNOWN(IParameterChanges)
    Queue queue;
    int32 PLUGIN_API getParameterCount() override { return queue.points.empty() ? 0 : 1; }
    IParamValueQueue* PLUGIN_API getParameterData(int32 index) override { return index == 0 ? &queue : nullptr; }
    IParamValueQueue* PLUGIN_API addParameterData(const ParamID& id, int32& index) override {
        queue.id = id; index = 0; return &queue;
    }
};
struct PlugFrame : IPlugFrame {
    HOST_UNKNOWN(IPlugFrame)
    tresult PLUGIN_API resizeView(IPlugView* view, ViewRect* size) override { return view->onSize(size); }
};
static void run_vst3(void* library, const char* bundle, bool gui) {
    NSURL* url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:bundle] isDirectory:YES];
    CFBundleRef cf_bundle = CFBundleCreate(kCFAllocatorDefault, (__bridge CFURLRef)url);
    auto enter = reinterpret_cast<bool (*)(CFBundleRef)>(dlsym(library, "bundleEntry"));
    auto exit = reinterpret_cast<bool (*)()>(dlsym(library, "bundleExit"));
    require(cf_bundle && enter && enter(cf_bundle), "VST3 bundle entry");
    auto get = reinterpret_cast<IPluginFactory* (*)()>(dlsym(library, "GetPluginFactory"));
    require(get, "VST3 factory symbol");
    auto* factory = get();
    require(factory && factory->countClasses() >= 1, "VST3 discovery");
    PClassInfo info{};
    bool found = false;
    for (int32 i = 0; i < factory->countClasses(); ++i) {
        require(factory->getClassInfo(i, &info) == kResultOk, "VST3 class metadata");
        if (!strcmp(info.category, kVstAudioEffectClass)) { found = true; break; }
    }
    require(found, "VST3 effect class");
    IComponent* component = nullptr;
    require(factory->createInstance(info.cid, IComponent::iid.toTUID(), (void**)&component) == kResultOk, "VST3 component");
    VstHost host;
    require(component->initialize(&host) == kResultOk, "VST3 initialize");
    IAudioProcessor* processor = nullptr;
    require(component->queryInterface(IAudioProcessor::iid.toTUID(), (void**)&processor) == kResultOk, "VST3 processor");
    IEditController* controller = nullptr;
    bool separate_controller = component->queryInterface(IEditController::iid.toTUID(), (void**)&controller) != kResultOk;
    if (separate_controller) {
        TUID controller_id{};
        require(component->getControllerClassId(controller_id) == kResultOk, "VST3 controller ID");
        require(factory->createInstance(controller_id, IEditController::iid.toTUID(), (void**)&controller) == kResultOk, "VST3 controller");
        require(controller->initialize(&host) == kResultOk, "VST3 controller initialize");
    }
    require(controller->getParameterCount() == 1, "VST3 parameter discovery");
    ParameterInfo parameter{};
    require(controller->getParameterInfo(0, parameter) == kResultOk, "VST3 parameter metadata");
    SpeakerArrangement stereo = SpeakerArr::kStereo;
    require(processor->setBusArrangements(&stereo, 1, &stereo, 1) == kResultOk, "VST3 stereo layout");
    ProcessSetup setup{}; setup.processMode = kRealtime; setup.symbolicSampleSize = kSample32;
    setup.maxSamplesPerBlock = 2048; setup.sampleRate = 48000;
    require(processor->setupProcessing(setup) == kResultOk, "VST3 setup");
    component->activateBus(kAudio, kInput, 0, true); component->activateBus(kAudio, kOutput, 0, true);
    require(component->setActive(true) == kResultOk, "VST3 activation");
    bool interpolated = true;
    struct Allocations { uint64_t allocated; uint64_t deallocated; } allocations{};
    auto begin_allocations = reinterpret_cast<void (*)()>(dlsym(library, "phase0_begin_allocations"));
    auto end_allocations = reinterpret_cast<Allocations (*)()>(dlsym(library, "phase0_end_allocations"));
    require(begin_allocations && end_allocations, "allocation counter exports");
    std::exception_ptr failure;
    std::thread processing([&] {
        audio_thread = true;
        try {
            require(processor->setProcessing(true) == kResultOk, "VST3 start");
            Samples samples;
            AudioBusBuffers in{}, out{};
            in.numChannels = out.numChannels = 2;
            in.channelBuffers32 = samples.in; out.channelBuffers32 = samples.out;
            Changes changes; changes.queue.id = parameter.id;
            ProcessData data{}; data.processMode = kRealtime; data.symbolicSampleSize = kSample32;
            data.numSamples = 64; data.numInputs = data.numOutputs = 1;
            data.inputs = &in; data.outputs = &out; data.inputParameterChanges = &changes;
            require(processor->process(data) == kResultOk, "VST3 pass-through");
            samples.check(64, 64, 1.0f);
            changes.queue.points = {{16, 1.0}, {17, 0.25}};
            require(processor->process(data) == kResultOk, "VST3 jump"); samples.check(64, 17, 0.25f);
            // A VST3 curve is a linear segment, not merely stepped changes at its points.
            changes.queue.points = {{0, 1.0}, {31, 0.0}};
            data.numSamples = 32;
            require(processor->process(data) == kResultOk, "VST3 automation curve");
            for (int i = 0; i < 32; ++i) {
                float expected = samples.left[i] * (1.0f - float(i)/31.0f);
                if (std::abs(samples.out_left[i] - expected) > 1e-6f) interpolated = false;
            }
            changes.queue.points.clear(); data.numSamples = 1;
            require(processor->process(data) == kResultOk, "VST3 variable block");
            samples.check(1, 0, 0.0f);
            // 2048 distinct, ordered points exceed this framework's default 1024-event reserve.
            // The host builds its queue before measuring only the plugin's audio-thread allocator.
            for (int i = 0; i < 2048; ++i) changes.queue.points.emplace_back(i, double(i)/2047.0);
            data.numSamples = 2048;
            begin_allocations();
            tresult result = processor->process(data);
            allocations = end_allocations();
            require(result == kResultOk, "VST3 dense legal automation");
            processor->setProcessing(false);
        } catch (...) { failure = std::current_exception(); }
    });
    processing.join(); if (failure) std::rethrow_exception(failure);
    int cycles = 0;
    if (gui) {
        auto* view = controller->createView(ViewType::kEditor);
        require(view && view->isPlatformTypeSupported(kPlatformTypeNSView) == kResultTrue, "VST3 Cocoa editor");
        PlugFrame frame; view->setFrame(&frame);
        NSWindow* w = window();
        for (int i = 0; i < 3; ++i) {
            require(view->attached((__bridge void*)[w contentView], kPlatformTypeNSView) == kResultOk, "VST3 editor embedding");
            pump(0.15);
            require([[[w contentView] subviews] count] > 0, "VST3 native child view missing");
            require(view->removed() == kResultOk, "VST3 editor detach"); pump(0.05); ++cycles;
        }
        view->setFrame(nullptr); view->release(); [w close];
    }
    component->setActive(false);
    if (separate_controller) controller->terminate();
    controller->release(); processor->release(); component->terminate(); component->release();
    factory->release(); require(exit && exit(), "VST3 bundle exit"); CFRelease(cf_bundle);
    std::cout << "{\"format\":\"vst3\",\"discovery\":true,\"stereo_buffers\":true,\"sample_offset_17\":true,\"curve_interpolated\":" << (interpolated ? "true" : "false") << ",\"dense_event_allocations\":" << allocations.allocated << ",\"dense_event_deallocations\":" << allocations.deallocated << ",\"variable_blocks\":true,\"editor_cycles\":" << cycles << "}\n";
}
int main(int argc, char** argv) {
    @autoreleasepool {
        try {
            require(argc == 4 || argc == 5, "usage: native-host clap|vst3 binary bundle [--gui]");
            require(argc != 5 || !strcmp(argv[4], "--gui"), "unknown option; only --gui is supported");
            void* library = dlopen(argv[2], RTLD_NOW | RTLD_LOCAL);
            if (!library) throw std::runtime_error(dlerror());
            if (!strcmp(argv[1], "clap")) run_clap(library, argv[3], argc == 5);
            else if (!strcmp(argv[1], "vst3")) run_vst3(library, argv[3], argc == 5);
            else throw std::runtime_error("unknown format");
            dlclose(library); return 0;
        } catch (const std::exception& error) {
            std::cerr << error.what() << '\n'; return 1;
        }
    }
}
