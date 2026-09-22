pub mod appearance;
pub mod avatar;
pub mod clone;
pub mod discovery;
pub mod launch;
pub mod local_state;
pub mod mutation;
pub mod profile;

pub use profile::ChromiumAdapter;

/// Every Chromium-family installation present on this machine.
pub fn discover() -> Vec<ChromiumAdapter> {
    discovery::discover_installs()
        .into_iter()
        .map(ChromiumAdapter::new)
        .collect()
}
