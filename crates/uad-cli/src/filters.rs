use clap::ValueEnum;
use uad_core::adb::PmListPacksFlag;
use uad_core::uad_lists::{Package, PackageState, Removal, UadList};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StateFilter {
    /// Show all packages regardless of state
    All,
    /// Show only enabled packages
    Enabled,
    /// Show only disabled packages
    Disabled,
    /// Show only uninstalled packages
    Uninstalled,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RemovalFilter {
    All,
    Recommended,
    Advanced,
    Expert,
    Unsafe,
    Unlisted,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ListFilter {
    All,
    Aosp,
    Carrier,
    Google,
    Misc,
    Oem,
    Pending,
    Unlisted,
}

impl StateFilter {
    pub fn to_pm_flag(self) -> Option<PmListPacksFlag> {
        match self {
            Self::Enabled => Some(PmListPacksFlag::OnlyEnabled),
            Self::Disabled => Some(PmListPacksFlag::OnlyDisabled),
            Self::Uninstalled | Self::All => Some(PmListPacksFlag::IncludeUninstalled),
        }
    }

    pub fn matches(self, pkg_state: PackageState) -> bool {
        match self {
            Self::All => true,
            Self::Enabled => pkg_state == PackageState::Enabled,
            Self::Disabled => pkg_state == PackageState::Disabled,
            Self::Uninstalled => pkg_state == PackageState::Uninstalled,
        }
    }

    pub fn is_specific(self) -> bool {
        matches!(self, Self::Enabled | Self::Disabled | Self::Uninstalled)
    }
}

impl RemovalFilter {
    pub fn matches(self, pkg_info: Option<&Package>) -> bool {
        match pkg_info {
            Some(info) => match self {
                Self::All => true,
                Self::Recommended => info.removal == Removal::Recommended,
                Self::Advanced => info.removal == Removal::Advanced,
                Self::Expert => info.removal == Removal::Expert,
                Self::Unsafe => info.removal == Removal::Unsafe,
                Self::Unlisted => info.removal == Removal::Unlisted,
            },
            None => matches!(self, Self::All | Self::Unlisted),
        }
    }

    pub fn is_specific(self) -> bool {
        !matches!(self, Self::All)
    }
}

impl ListFilter {
    pub fn matches(self, pkg_info: Option<&Package>) -> bool {
        match pkg_info {
            Some(info) => match self {
                Self::All => true,
                Self::Aosp => info.list == UadList::Aosp,
                Self::Carrier => info.list == UadList::Carrier,
                Self::Google => info.list == UadList::Google,
                Self::Misc => info.list == UadList::Misc,
                Self::Oem => info.list == UadList::Oem,
                Self::Pending => info.list == UadList::Pending,
                Self::Unlisted => info.list == UadList::Unlisted,
            },
            None => matches!(self, Self::All | Self::Unlisted),
        }
    }
}
