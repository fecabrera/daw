//! Main-thread broker for worker preparation. No RPC occurs on an audio callback.
use crate::*;
use daw_unit_core::HostedSettings;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc,
};

/// Audio publishes the instance that actually owns the audible graph. Prepared
/// replacements do not redirect editor windows before renderer publication.
#[derive(Default)]
pub struct ActiveInstance(AtomicU64);
impl ActiveInstance {
    pub fn token(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
    pub fn publish(&self, token: u64) {
        self.0.store(token, Ordering::Release);
    }
}
pub struct PreparedInstance {
    pub processor: NativeProcessor,
    pub metadata: NativeMetadata,
    pub values: Vec<f64>,
    pub token: u64,
    pub active: Arc<ActiveInstance>,
}
struct Request {
    offline: bool,
    settings: HostedSettings,
    rate: u32,
    maximum: u32,
    state: Option<NativeState>,
    response: mpsc::SyncSender<Result<PreparedInstance>>,
}
/// Clone this endpoint into immutable render plans. Calling prepare waits for the
/// application main thread and is legal only during worker preparation.
#[derive(Clone)]
pub struct HostClient {
    requests: mpsc::SyncSender<Request>,
    running: Arc<AtomicBool>,
    main_thread: std::thread::ThreadId,
    installs: mpsc::SyncSender<InstallRequest>,
    missing: std::collections::HashSet<PluginIdentity>,
    degraded: bool,
}
pub struct InstalledPlugin {
    pub settings: HostedSettings,
    pub states: StateBank,
    pub metadata: NativeMetadata,
}
struct InstallRequest {
    identity: PluginIdentity,
    rate: u32,
    maximum: u32,
    response: mpsc::SyncSender<Result<InstalledPlugin>>,
}
impl HostClient {
    /// Explicit offline policy. This list comes from the scanned registry; a
    /// processing/state failure of an available plugin never enters this path.
    pub fn allow_missing(&self, identities: impl IntoIterator<Item = PluginIdentity>) -> Self {
        let mut client = self.clone();
        client.missing = identities.into_iter().collect();
        client.degraded = true;
        client
    }
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }
    pub fn is_missing(&self, identity: &PluginIdentity) -> bool {
        self.missing.contains(identity)
    }

    /// Queue a user-requested installation; polling the receipt never blocks UI.
    pub fn install(
        &self,
        identity: PluginIdentity,
        rate: u32,
        maximum: u32,
    ) -> Result<mpsc::Receiver<Result<InstalledPlugin>>> {
        let (response, receiver) = mpsc::sync_channel(1);
        self.installs
            .try_send(InstallRequest {
                identity,
                rate,
                maximum,
                response,
            })
            .map_err(|_| Error::Invalid("Plugin preparation queue is full or stopped".into()))?;
        Ok(receiver)
    }

    pub fn prepare(
        &self,
        settings: &HostedSettings,
        rate: u32,
        maximum: u32,
        state: Option<NativeState>,
    ) -> Result<PreparedInstance> {
        self.prepare_mode(settings, rate, maximum, state, false)
    }
    pub fn prepare_mode(
        &self,
        settings: &HostedSettings,
        rate: u32,
        maximum: u32,
        state: Option<NativeState>,
        offline: bool,
    ) -> Result<PreparedInstance> {
        if std::thread::current().id() == self.main_thread {
            return Err(Error::Invalid("Hosted renderer preparation must run on a worker while the main thread pumps native services".into()));
        }
        if !settings.valid() {
            return Err(Error::Invalid("Invalid hosted unit settings".into()));
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut request = Request {
            offline,
            settings: settings.clone(),
            rate,
            maximum,
            state,
            response: sender,
        };
        loop {
            if !self.running.load(Ordering::Acquire) {
                return Err(Error::Invalid(
                    "Plugin main-thread service has stopped".into(),
                ));
            }
            match self.requests.try_send(request) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Full(pending)) => {
                    request = pending;
                    std::thread::park_timeout(std::time::Duration::from_millis(1));
                }
                Err(_) => return Err(Error::Invalid("Plugin preparation was interrupted".into())),
            }
        }
        loop {
            if !self.running.load(Ordering::Acquire) {
                return Err(Error::Invalid(
                    "Plugin main-thread service has stopped".into(),
                ));
            }
            match receiver.recv_timeout(std::time::Duration::from_millis(10)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err(Error::Invalid("Plugin preparation was interrupted".into())),
            }
        }
    }
}
pub struct HostService {
    pub native: NativeHost,
    pub registry: PluginRegistry,
    requests: mpsc::Receiver<Request>,
    links: std::collections::HashMap<uuid::Uuid, Arc<ActiveInstance>>,
    instances: std::collections::HashMap<u64, (uuid::Uuid, uuid::Uuid)>,
    next_token: u64,
    running: Arc<AtomicBool>,
    installs: mpsc::Receiver<InstallRequest>,
    controls: std::collections::HashMap<uuid::Uuid, uuid::Uuid>,
}
impl HostService {
    pub fn new(registry: PluginRegistry) -> (Self, Arc<HostClient>) {
        let running = Arc::new(AtomicBool::new(true));
        let (sender, receiver) = mpsc::sync_channel(16);
        let (install_sender, installs) = mpsc::sync_channel(16);
        (
            Self {
                native: NativeHost::new(),
                registry,
                requests: receiver,
                links: Default::default(),
                instances: Default::default(),
                next_token: 1,
                running: running.clone(),
                installs,
                controls: Default::default(),
            },
            Arc::new(HostClient {
                running,
                installs: install_sender,
                requests: sender,
                main_thread: std::thread::current().id(),
                missing: Default::default(),
                degraded: false,
            }),
        )
    }
    pub fn shutdown(&mut self) {
        self.running.store(false, Ordering::Release);
        while let Ok(request) = self.requests.try_recv() {
            let _ = request.response.send(Err(Error::Invalid(
                "Plugin service is shutting down".into(),
            )));
        }
        while let Ok(request) = self.installs.try_recv() {
            let _ = request.response.send(Err(Error::Invalid(
                "Plugin service is shutting down".into(),
            )));
        }
    }
    /// Call on the application main thread. The callback receives project unit
    /// identity, rather than a pending renderer's private instance identity.
    pub fn pump(&mut self, mut notify: impl FnMut(uuid::Uuid, NativeNotification)) {
        if let Ok(request) = self.installs.try_recv() {
            let result = self.install(&request);
            let _ = request.response.send(result);
        }
        // Admit at most one renderer initializer per frame; constructors can take time.
        if let Ok(request) = self.requests.try_recv() {
            let result = self.prepare(&request);
            if let Err(mpsc::SendError(Ok(prepared))) = request.response.send(result) {
                drop(prepared); // unique lease returned; native storage retires below
            }
        }
        self.native.pump(|id, event| {
            if let Some((&token, &(unit, _))) =
                self.instances.iter().find(|(_, (_, native))| *native == id)
                && self
                    .links
                    .get(&unit)
                    .is_some_and(|link| link.token() == token)
            {
                notify(unit, event);
            } else if let Some((&unit, _)) = self.controls.iter().find(|(_, native)| **native == id)
            {
                notify(unit, event);
            }
        });
        self.instances
            .retain(|_, (_, native)| self.native.metadata(*native).is_ok());
    }
    fn install(&mut self, request: &InstallRequest) -> Result<InstalledPlugin> {
        let (candidate, descriptor) = self
            .registry
            .resolve(&request.identity)
            .ok_or_else(|| Error::Invalid("Plugin is missing or quarantined".into()))?;
        let descriptor = descriptor.clone();
        let id = unsafe {
            self.native.load(
                candidate,
                &descriptor.identity,
                request.rate,
                request.maximum,
                None,
            )
        }?;
        let result = (|| -> Result<InstalledPlugin> {
            let metadata = self.native.metadata(id)?.clone();
            let parameters = metadata
                .parameters
                .iter()
                .map(|definition| {
                    self.native.parameter_value(id, definition.id).map(|base| {
                        daw_unit_core::HostedParameterBinding::new(definition.clone(), base)
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            let mut settings = HostedSettings::new(descriptor, parameters);
            settings.metadata_rate = request.rate;
            settings.output_channels = metadata.outputs[0].channels;
            settings.latency_frames = metadata.latency_frames;
            settings.tail_frames = metadata.tail_frames;
            let mut states = StateBank::default();
            if metadata.state_supported {
                settings.state = Some(states.capture(self.native.save_state(id)?, 0)?);
            }
            if !settings.valid() {
                return Err(Error::Invalid(
                    "Plugin exceeds the supported unit or latency bounds".into(),
                ));
            }
            Ok(InstalledPlugin {
                settings,
                states,
                metadata,
            })
        })();
        self.native.remove(id)?;
        result
    }
    fn prepare(&mut self, request: &Request) -> Result<PreparedInstance> {
        let (candidate, descriptor) = self
            .registry
            .resolve(&request.settings.plugin.identity)
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "Plugin is missing or quarantined: {}",
                    request.settings.plugin.name
                ))
            })?;
        if descriptor.role != request.settings.plugin.role {
            return Err(Error::Invalid("Installed plugin role changed".into()));
        }
        // SAFETY: service belongs to application main; scan admission established the ABI.
        let id = unsafe {
            self.native.load_configured(
                candidate,
                &descriptor.identity,
                NativeConfiguration {
                    sample_rate_hz: request.rate,
                    max_block_frames: request.maximum,
                    offline: request.offline,
                },
                request.state.as_ref(),
            )
        }?;
        let result = (|| -> Result<PreparedInstance> {
            let metadata = self.native.metadata(id)?.clone();
            // A changed layout must be reviewed on the control thread, never guessed on audio.
            if metadata.parameters.len() != request.settings.parameters.len()
                || request.settings.parameters.iter().any(|binding| {
                    !metadata
                        .parameters
                        .iter()
                        .any(|p| p.id == binding.definition.id && *p == binding.definition)
                })
            {
                return Err(Error::Invalid(
                    "Plugin parameter layout changed; reopen the unit to refresh its mapping"
                        .into(),
                ));
            }
            for binding in &request.settings.parameters {
                if request.state.is_some()
                    && binding.edit_revision
                        > request.settings.state.as_ref().map_or(0, |s| s.revision)
                    && !binding.definition.read_only
                {
                    self.native
                        .set_parameter(id, binding.definition.id, binding.base_value)?;
                }
            }
            let values = request
                .settings
                .parameters
                .iter()
                .map(|p| {
                    if request.state.is_some()
                        && p.edit_revision
                            > request.settings.state.as_ref().map_or(0, |s| s.revision)
                    {
                        Ok(p.base_value)
                    } else {
                        self.native.parameter_value(id, p.definition.id)
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            let processor = self.native.take_processor(id)?;
            let active = self.links.entry(request.settings.id).or_default().clone();
            let token = self.next_token;
            self.next_token = self
                .next_token
                .checked_add(1)
                .ok_or_else(|| Error::Invalid("Plugin instance token exhausted".into()))?;
            self.instances.insert(token, (request.settings.id, id));
            Ok(PreparedInstance {
                processor,
                metadata,
                values,
                token,
                active,
            })
        })();
        // The broker retains storage until the exclusive processor comes back.
        if result.is_ok() {
            self.native.retire_when_returned(id)?;
        } else {
            self.native.remove(id)?;
        }
        result
    }
    /// A stopped editor can own an inactive control instance without an audio
    /// device. It never masquerades as a playable DSP state or a clone.
    pub fn control_instance(
        &mut self,
        settings: &HostedSettings,
        states: &StateBank,
    ) -> Result<uuid::Uuid> {
        if let Some(id) = self.active_native(settings.id) {
            return Ok(id);
        }
        if let Some(&id) = self.controls.get(&settings.id) {
            return Ok(id);
        }
        let (candidate, descriptor) = self
            .registry
            .resolve(&settings.plugin.identity)
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "Plugin is missing or quarantined: {}",
                    settings.plugin.name
                ))
            })?;
        let state = settings
            .state
            .as_ref()
            .map(|s| states.restore(settings.plugin.identity.format(), s))
            .transpose()?;
        let id = unsafe {
            self.native.load(
                candidate,
                &descriptor.identity,
                settings.metadata_rate,
                4096,
                state.as_ref(),
            )
        }?;
        for binding in &settings.parameters {
            if state.is_some()
                && binding.edit_revision > settings.state.as_ref().map_or(0, |s| s.revision)
                && !binding.definition.read_only
            {
                self.native
                    .set_parameter(id, binding.definition.id, binding.base_value)?;
            }
        }
        self.controls.insert(settings.id, id);
        Ok(id)
    }
    pub fn retain_units(&mut self, units: &std::collections::HashSet<uuid::Uuid>) {
        let removed: Vec<_> = self
            .controls
            .iter()
            .filter(|(unit, _)| !units.contains(unit))
            .map(|(unit, native)| (*unit, *native))
            .collect();
        for (unit, native) in removed {
            let _ = self.native.remove(native);
            self.controls.remove(&unit);
        }
        self.links.retain(|unit, _| units.contains(unit));
    }
    pub fn reopen(&mut self, unit: uuid::Uuid) -> Result<()> {
        if let Some(id) = self.current_native(unit) {
            self.native.invalidate(id)?;
        }
        if let Some(link) = self.links.get(&unit) {
            link.publish(0);
        }
        if let Some(id) = self.controls.remove(&unit) {
            self.native.remove(id)?;
        }
        Ok(())
    }
    pub fn current_native(&self, unit: uuid::Uuid) -> Option<uuid::Uuid> {
        self.active_native(unit)
            .or_else(|| self.controls.get(&unit).copied())
    }
    pub fn active_native(&self, unit: uuid::Uuid) -> Option<uuid::Uuid> {
        self.links
            .get(&unit)
            .and_then(|l| self.instances.get(&l.token()))
            .map(|(_, native)| *native)
    }
    /// Run a CLI worker while preserving SDK main-thread services.
    pub fn run<T: Send>(&mut self, task: impl FnOnce() -> T + Send) -> T {
        std::thread::scope(|scope| {
            let worker = scope.spawn(task);
            while !worker.is_finished() {
                self.pump(|_, _| {});
                crate::native::pump_application_events();
                std::thread::park_timeout(std::time::Duration::from_millis(1));
            }
            let result = worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            self.pump(|_, _| {});
            result
        })
    }
}

impl Drop for HostService {
    fn drop(&mut self) {
        self.shutdown();
    }
}
