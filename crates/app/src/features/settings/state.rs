use gd_ui::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SettingsPage {
    #[default]
    Appearance,
    Diff,
    Accounts,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 3] = [Self::Appearance, Self::Diff, Self::Accounts];

    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Diff => "Diff",
            Self::Accounts => "Accounts",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Appearance => Icon::Eye,
            Self::Diff => Icon::Tuning,
            Self::Accounts => Icon::Globe,
        }
    }
}
