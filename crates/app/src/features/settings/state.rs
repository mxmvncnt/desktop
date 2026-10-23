use gd_ui::Icon;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SettingsPage {
    #[default]
    Appearance,
    Diff,
}

impl SettingsPage {
    pub const ALL: [SettingsPage; 2] = [Self::Appearance, Self::Diff];

    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Diff => "Diff",
        }
    }

    pub fn icon(self) -> Icon {
        match self {
            Self::Appearance => Icon::Eye,
            Self::Diff => Icon::Tuning,
        }
    }
}
