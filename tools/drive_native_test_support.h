// BSD-2-Clause. Shared fixtures for the separate macOS probe executables.
#pragma once

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

// The host owns these stack objects until after plugin termination.
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
