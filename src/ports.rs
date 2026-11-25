//! Server implementations of component ports whose owners a component may not depend on, registered once at startup.
pub(crate) mod apps;
pub(crate) mod inference;

/// Registers every process-wide port implementation; repeated calls keep the first registration.
pub(crate) fn install() {
    veyndo_cognition::ports::install(Box::new(inference::Host));
    veyndo_app_platform::ports::install_directory(Box::new(apps::Directory));
}
