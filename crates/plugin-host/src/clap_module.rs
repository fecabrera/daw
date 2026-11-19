use crate::{Error, PluginFormat, Result};
use daw_plugin_clap::sys::{
    entry::clap_plugin_entry,
    factory::plugin_factory::{CLAP_PLUGIN_FACTORY_ID, clap_plugin_factory},
    version::clap_version_is_compatible,
};
use libloading::Library;
use std::path::Path;

#[derive(Clone, Copy)]
pub(crate) enum Use {
    Scan,
    Host,
}

pub(crate) struct ClapModule {
    deinit: unsafe extern "C" fn(),
    factory: *const clap_plugin_factory,
    _library: Library,
}

impl ClapModule {
    /// Keep the module on the caller's control thread and drop its instances first.
    pub(crate) unsafe fn open(path: &Path, usage: Use) -> Result<Self> {
        let scanning = matches!(usage, Use::Scan);
        let library = unsafe { Library::new(crate::module_executable(PluginFormat::Clap, path)?) }?;
        let entry = unsafe { library.get::<*const clap_plugin_entry>(b"clap_entry\0") }?;
        let entry =
            unsafe { (*entry).as_ref() }.ok_or_else(|| Error::Invalid("Null CLAP entry".into()))?;
        let invalid = |message: &str| Error::Invalid(message.into());
        if !clap_version_is_compatible(entry.clap_version) || (!scanning && entry.deinit.is_none())
        {
            return Err(invalid(if scanning {
                "Incompatible CLAP version"
            } else {
                "Incompatible CLAP module"
            }));
        }
        let factory_lookup = || {
            entry
                .get_factory
                .ok_or_else(|| invalid("Missing CLAP factory lookup"))
        };
        let init = if scanning {
            let init = entry.init.ok_or_else(|| invalid("Missing CLAP init"))?;
            if entry.deinit.is_none() {
                return Err(invalid("Missing CLAP deinit"));
            }
            factory_lookup()?;
            Some(init)
        } else {
            entry.init
        };
        let path = crate::scan::path_cstring(path)?;
        let init = init.ok_or_else(|| invalid("Missing CLAP initializer"))?;
        if !unsafe { init(path.as_ptr()) } {
            return Err(invalid(if scanning {
                "CLAP initialization failed"
            } else {
                "CLAP module initialization failed"
            }));
        }
        let lookup = entry.get_factory;
        let mut module = Self {
            deinit: entry.deinit.unwrap(),
            factory: std::ptr::null(),
            _library: library,
        };
        let lookup = lookup.ok_or_else(|| invalid("Missing CLAP factory lookup"))?;
        module.factory = unsafe { lookup(CLAP_PLUGIN_FACTORY_ID.as_ptr()) }.cast();
        if module.factory.is_null() {
            return Err(invalid("Missing CLAP plugin factory"));
        }
        Ok(module)
    }

    pub(crate) fn factory(&self) -> &clap_plugin_factory {
        // SAFETY: open checked the initialized module's factory pointer.
        unsafe { &*self.factory }
    }
}

impl Drop for ClapModule {
    fn drop(&mut self) {
        // SAFETY: only an initialized module owns this callback. The library is still loaded.
        unsafe { (self.deinit)() };
    }
}
