pub mod discovery;
pub mod local_state;
pub mod profile;

pub use profile::ChromiumAdapter;

/// Every Chromium-family installation present on this machine.
pub fn discover() -> Vec<ChromiumAdapter> {
    discovery::discover_installs()
        .into_iter()
        .map(ChromiumAdapter::new)
        .collect()
}
