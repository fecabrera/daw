// BSD-2-Clause. Native Drive editor acceptance host; development artifacts only.
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
#include <functional>
#include <fstream>

using namespace Steinberg;
using namespace Steinberg::Vst;
static thread_local bool audio_thread = false;
static void require(bool ok, const char* message) {
    if (!ok) { std::cerr << (message ? message : "unknown native failure") << std::endl; throw std::runtime_error(message ? message : "unknown native failure"); }
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
    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
    if(!NSApp.running)[NSApp finishLaunching];
    NSWindow* w = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 180, 218)
        styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
    [w setTitle:@"DAW Drive Phase 4 editor"];
    [w setReleasedWhenClosed:NO];
    [w setAcceptsMouseMovedEvents:YES];
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
static std::function<void()> main_tick;
static void run_for(double seconds) {
    auto end=std::chrono::steady_clock::now()+std::chrono::duration<double>(seconds);
    while(std::chrono::steady_clock::now()<end){ if(main_tick)main_tick(); pump(0.01); }
}
static void mouse(NSWindow* w,NSEventType type,double x,double y) {
    NSEvent* e=[NSEvent mouseEventWithType:type location:NSMakePoint(x,218-y) modifierFlags:0
        timestamp:[NSProcessInfo processInfo].systemUptime windowNumber:w.windowNumber context:nil eventNumber:0 clickCount:1 pressure:1];
    [w sendEvent:e];run_for(0.05);
}
static void drag(NSWindow* w,double x,double y) {
    [w makeKeyAndOrderFront:nil];[NSApp activateIgnoringOtherApps:YES];run_for(0.1);
    require(w.keyWindow && NSApp.active,"native input requires the test window to have focus");
    bool shape=x>100&&y==36;double endx=x+(shape?40:0),endy=y-(shape?0:18);
    mouse(w,NSEventTypeMouseMoved,x,y);mouse(w,NSEventTypeLeftMouseDown,x,y);mouse(w,NSEventTypeLeftMouseDragged,endx,endy);mouse(w,NSEventTypeLeftMouseUp,endx,endy);
}
static void exercise(NSWindow* w) {
    // Off retains and disables the bottom two Color controls. The format host
    // separately enables Color before exercising the six active knobs.
    drag(w,45,36);drag(w,45,96);drag(w,45,156);drag(w,135,36);drag(w,135,96);drag(w,135,156);
}
struct Counts {
    std::array<std::atomic<int>,7> begin{},value{},end{};
    std::array<std::atomic<bool>,7> active{};
    std::atomic<bool> order_ok{true};
    void event(int id,int kind) {
        require(id>=100 && id<=106,"unknown editor parameter"); int i=id-100;
        if(kind==0){if(active[i].exchange(true))order_ok=false;++begin[i];}
        else if(kind==1){if(!active[i])order_ok=false;++value[i];}
        else {if(!active[i].exchange(false))order_ok=false;++end[i];}
    }
    void check() {
        require(order_ok,"gesture order");
        for(int i=1;i<7;++i)std::cerr<<"param "<<100+i<<" begin="<<begin[i]<<" value="<<value[i]<<" end="<<end[i]<<std::endl;
        for(int i=1;i<7;++i){if(value[i]==0)std::cerr<<"Missing edit for parameter "<<100+i<<std::endl;require(value[i]>0,"missing knob edit");require(begin[i]==end[i] && !active[i],"unbalanced gestures");}
    }
};
struct ClapContext {
    std::atomic<bool> callback{false};Counts counts;
};
static const void* CLAP_ABI extension(const clap_host_t*,const char* id){
    static const clap_host_params_t params={[](const clap_host_t*,uint32_t){},[](const clap_host_t*,clap_id,uint32_t){},[](const clap_host_t*){}};
    if(!strcmp(id,CLAP_EXT_PARAMS))return &params;return nullptr;
}
static void CLAP_ABI request_callback(const clap_host_t* h){static_cast<ClapContext*>(h->host_data)->callback=true;}
static void CLAP_ABI noop(const clap_host_t*){}
static void clap(void* library,const char* bundle,int cycles,double hold){
    auto* entry=static_cast<const clap_plugin_entry_t*>(dlsym(library,"clap_entry"));require(entry&&entry->init(bundle),"CLAP entry");
    auto* factory=static_cast<const clap_plugin_factory_t*>(entry->get_factory(CLAP_PLUGIN_FACTORY_ID));require(factory,"CLAP factory");
    ClapContext context;
    clap_host_t host{CLAP_VERSION,&context,"Drive editor test","Sie","","0.1",extension,noop,noop,request_callback};
    const auto* plugin=factory->create_plugin(factory,&host,factory->get_plugin_descriptor(factory,0)->id);require(plugin&&plugin->init(plugin),"CLAP instance");
    auto* gui=static_cast<const clap_plugin_gui_t*>(plugin->get_extension(plugin,CLAP_EXT_GUI));require(gui,"CLAP GUI");
    require(plugin->activate(plugin,48000,1,64),"CLAP activate");
    std::atomic<bool> stop{false};std::atomic<int> blocks{0};
    std::thread audio([&]{audio_thread=true;require(plugin->start_processing(plugin),"CLAP start");Samples samples;
        clap_audio_buffer_t input{samples.in,nullptr,2,0,0},output{samples.out,nullptr,2,0,0};
        clap_output_events_t events{&context,[](const clap_output_events_t* out,const clap_event_header_t* e)->bool{
            auto& counts=static_cast<ClapContext*>(out->ctx)->counts;
            if(e->type==CLAP_EVENT_PARAM_VALUE){auto* value=reinterpret_cast<const clap_event_param_value_t*>(e);counts.event(value->param_id,1);}
            else if(e->type==CLAP_EVENT_PARAM_GESTURE_BEGIN||e->type==CLAP_EVENT_PARAM_GESTURE_END)
                counts.event(reinterpret_cast<const clap_event_param_gesture_t*>(e)->param_id,e->type==CLAP_EVENT_PARAM_GESTURE_BEGIN?0:2);
            return true;}};
        clap_event_param_value_t color{{sizeof(clap_event_param_value_t),0,CLAP_CORE_EVENT_SPACE_ID,CLAP_EVENT_PARAM_VALUE,0},104,nullptr,-1,-1,-1,-1,1.0};
        clap_input_events_t incoming{&color,[](const clap_input_events_t*)->uint32_t{return 1;},[](const clap_input_events_t* in,uint32_t)->const clap_event_header_t*{return &static_cast<clap_event_param_value_t*>(in->ctx)->header;}};
        clap_process_t p{};p.frames_count=64;p.audio_inputs=&input;p.audio_outputs=&output;p.audio_inputs_count=p.audio_outputs_count=1;p.out_events=&events;p.in_events=&incoming;
        while(!stop){require(plugin->process(plugin,&p)!=CLAP_PROCESS_ERROR,"CLAP process with editor");p.in_events=nullptr;++blocks;
            for(float x:samples.out_left)require(std::isfinite(x),"CLAP finite output");std::this_thread::sleep_for(std::chrono::milliseconds(1));}
        plugin->stop_processing(plugin);
    });
    main_tick=[&]{if(context.callback.exchange(false))plugin->on_main_thread(plugin);};
    NSWindow* w=window();[w makeKeyAndOrderFront:nil];[NSApp activateIgnoringOtherApps:YES];
    for(int cycle=0;cycle<cycles;++cycle){require(gui->create(plugin,CLAP_WINDOW_API_COCOA,false),"CLAP create editor");
        uint32_t width=0,height=0;require(gui->get_size(plugin,&width,&height)&&width==180&&height==218,"CLAP body size");
        clap_window_t parent{};parent.api=CLAP_WINDOW_API_COCOA;parent.cocoa=(__bridge void*)w.contentView;
        require(gui->set_parent(plugin,&parent)&&gui->show(plugin),"CLAP attach/show");run_for(.3);
        require(w.contentView.subviews.count>0,"CLAP native child");if(cycle==0){exercise(w);
        }run_for(hold);
        require(gui->hide(plugin),"CLAP hide");gui->destroy(plugin);run_for(.1);require(w.contentView.subviews.count==0,"CLAP child retirement");}
    stop=true;audio.join();main_tick={};[w close];context.counts.check();require(blocks>100,"CLAP concurrent processing");
    plugin->deactivate(plugin);plugin->destroy(plugin);entry->deinit();
    std::cout<<"{\"format\":\"clap\",\"editor_cycles\":"<<cycles<<",\"audio_blocks\":"<<blocks<<",\"six_controls\":true,\"balanced_gestures\":true}\n";
}
struct Handler:IComponentHandler {
    HOST_UNKNOWN(IComponentHandler)
    Counts counts;
    std::array<std::atomic<double>,7> values{};
    std::array<std::atomic<bool>,7> pending{};
    tresult PLUGIN_API beginEdit(ParamID id)override{require([NSThread isMainThread],"VST begin main thread");counts.event(id,0);return kResultOk;}
    tresult PLUGIN_API performEdit(ParamID id,ParamValue value)override{require([NSThread isMainThread],"VST perform main thread");counts.event(id,1);values[id-100]=value;pending[id-100]=true;return kResultOk;}
    tresult PLUGIN_API endEdit(ParamID id)override{require([NSThread isMainThread],"VST end main thread");counts.event(id,2);return kResultOk;}
    tresult PLUGIN_API restartComponent(int32)override{return kResultOk;}
};
static void vst(void* library,const char* bundle,int cycles,double hold){
    auto enter=reinterpret_cast<bool(*)(CFBundleRef)>(dlsym(library,"bundleEntry"));auto exit=reinterpret_cast<bool(*)()>(dlsym(library,"bundleExit"));
    NSURL* url=[NSURL fileURLWithPath:[NSString stringWithUTF8String:bundle] isDirectory:YES];auto cf=CFBundleCreate(kCFAllocatorDefault,(__bridge CFURLRef)url);
    require(enter&&enter(cf),"VST entry");auto get=reinterpret_cast<IPluginFactory*(*)()>(dlsym(library,"GetPluginFactory"));auto* factory=get();
    PClassInfo info{};bool found=false;for(int i=0;i<factory->countClasses();++i){factory->getClassInfo(i,&info);if(!strcmp(info.category,kVstAudioEffectClass)){found=true;break;}}require(found,"VST processor class");
    IComponent* component=nullptr;require(factory->createInstance(info.cid,IComponent::iid.toTUID(),(void**)&component)==kResultOk,"VST component");VstHost host;require(component->initialize(&host)==kResultOk,"VST init");
    IAudioProcessor* processor=nullptr;require(component->queryInterface(IAudioProcessor::iid.toTUID(),(void**)&processor)==kResultOk,"VST processor");
    TUID cid{};component->getControllerClassId(cid);IEditController* controller=nullptr;require(factory->createInstance(cid,IEditController::iid.toTUID(),(void**)&controller)==kResultOk,"VST controller");controller->initialize(&host);
    Handler handler;controller->setComponentHandler(&handler);controller->setParamNormalized(104,1.0/3.0);
    ProcessSetup setup{kRealtime,kSample32,64,48000.0};require(processor->setupProcessing(setup)==kResultOk,"VST setup");component->activateBus(kAudio,kInput,0,true);component->activateBus(kAudio,kOutput,0,true);component->setActive(true);
    std::atomic<bool> stop{false};std::atomic<int> blocks{0};
    std::thread audio([&]{audio_thread=true;processor->setProcessing(true);Samples samples;AudioBusBuffers in{},out{};in.numChannels=out.numChannels=2;in.channelBuffers32=samples.in;out.channelBuffers32=samples.out;
        Changes changes;changes.queue.id=104;changes.queue.points={{0,1.0/3.0}};
        ProcessData p{};p.processMode=kRealtime;p.symbolicSampleSize=kSample32;p.numSamples=64;p.numInputs=p.numOutputs=1;p.inputs=&in;p.outputs=&out;p.inputParameterChanges=&changes;
        require(processor->process(p)==kResultOk,"VST initial shape");
        while(!stop){
            for(int i=1;i<7;++i)if(handler.pending[i].exchange(false)){changes.queue.id=100+i;changes.queue.points={{0,handler.values[i]}};require(processor->process(p)==kResultOk,"VST UI queue");}
            changes.queue.points.clear();require(processor->process(p)==kResultOk,"VST process with editor");++blocks;for(float x:samples.out_left)require(std::isfinite(x),"VST finite output");std::this_thread::sleep_for(std::chrono::milliseconds(1));}
        processor->setProcessing(false);
    });
    NSWindow* w=window();[w makeKeyAndOrderFront:nil];[NSApp activateIgnoringOtherApps:YES];PlugFrame frame;
    for(int cycle=0;cycle<cycles;++cycle){auto* view=controller->createView(ViewType::kEditor);require(view,"VST custom editor");view->setFrame(&frame);ViewRect rect{};require(view->getSize(&rect)==kResultOk&&rect.right==180&&rect.bottom==218,"VST body size");
        require(view->attached((__bridge void*)w.contentView,kPlatformTypeNSView)==kResultOk,"VST attach");run_for(.3);require(w.contentView.subviews.count>0,"VST native child");if(cycle==0){exercise(w);
        }run_for(hold);
        require(view->removed()==kResultOk,"VST remove");view->setFrame(nullptr);view->release();run_for(.1);require(w.contentView.subviews.count==0,"VST child retirement");}
    stop=true;audio.join();[w close];handler.counts.check();require(blocks>100,"VST concurrent processing");component->setActive(false);controller->setComponentHandler(nullptr);controller->terminate();controller->release();processor->release();component->terminate();component->release();factory->release();exit();CFRelease(cf);
    std::cout<<"{\"format\":\"vst3\",\"editor_cycles\":"<<cycles<<",\"audio_blocks\":"<<blocks<<",\"six_controls\":true,\"balanced_gestures\":true}\n";
}
int main(int argc,char** argv){@autoreleasepool{try{
    std::vector<std::string> config;std::vector<char*> args;
    if(argc==1){
        NSString* path=[[NSBundle mainBundle] pathForResource:@"host-args" ofType:@"txt"];
        require(path,"test app configuration");std::ifstream input(path.UTF8String);std::string line;
        config.push_back("native-host");while(std::getline(input,line))config.push_back(line);
        for(auto& value:config)args.push_back(value.data());argc=args.size();argv=args.data();
    }
    require(argc>=4,"native-host clap|vst3 binary bundle [hold-seconds]");void* library=dlopen(argv[2],RTLD_NOW|RTLD_LOCAL);require(library,dlerror());double hold=argc>4?std::stod(argv[4]):0.1;
    if(!strcmp(argv[1],"clap"))clap(library,argv[3],3,hold);else vst(library,argv[3],3,hold);
    dlclose(library);return 0;
}catch(const std::exception& e){std::cerr<<e.what()<<"\n";return 1;}}}
