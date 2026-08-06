use eframe::egui::Color32;

#[derive(Clone)]
pub struct Workspace {
    pub name: &'static str,
    pub initials: &'static str,
    pub color: Color32,
}

#[derive(Clone)]
pub struct Conversation {
    pub name: &'static str,
    pub preview: &'static str,
    pub initials: &'static str,
    pub color: Color32,
    pub unread: u32,
    pub online: bool,
    pub pinned: bool,
}

#[derive(Clone)]
pub struct Message {
    pub author: &'static str,
    pub initials: &'static str,
    pub color: Color32,
    pub time: &'static str,
    pub body: String,
    pub mine: bool,
}

pub fn workspaces() -> Vec<Workspace> {
    vec![
        Workspace {
            name: "Home",
            initials: "C",
            color: Color32::from_rgb(112, 132, 255),
        },
        Workspace {
            name: "Design guild",
            initials: "DG",
            color: Color32::from_rgb(225, 109, 127),
        },
        Workspace {
            name: "Night shift",
            initials: "NS",
            color: Color32::from_rgb(92, 169, 142),
        },
        Workspace {
            name: "Open source",
            initials: "OS",
            color: Color32::from_rgb(210, 145, 75),
        },
        Workspace {
            name: "Book club",
            initials: "BC",
            color: Color32::from_rgb(154, 117, 204),
        },
    ]
}

pub fn conversations() -> Vec<Conversation> {
    vec![
        Conversation {
            name: "Juniper",
            preview: "That sounds perfect — ship it.",
            initials: "JU",
            color: Color32::from_rgb(216, 123, 151),
            unread: 2,
            online: true,
            pinned: true,
        },
        Conversation {
            name: "Product room",
            preview: "Mina: New mockups are ready",
            initials: "PR",
            color: Color32::from_rgb(91, 157, 212),
            unread: 0,
            online: true,
            pinned: true,
        },
        Conversation {
            name: "Theo Martinez",
            preview: "You: I’ll take a look tonight",
            initials: "TM",
            color: Color32::from_rgb(208, 146, 86),
            unread: 0,
            online: false,
            pinned: false,
        },
        Conversation {
            name: "Indie makers",
            preview: "Oren: anyone up for a review?",
            initials: "IM",
            color: Color32::from_rgb(111, 176, 122),
            unread: 4,
            online: true,
            pinned: false,
        },
        Conversation {
            name: "Nova",
            preview: "Sent an attachment",
            initials: "NO",
            color: Color32::from_rgb(148, 124, 214),
            unread: 0,
            online: true,
            pinned: false,
        },
        Conversation {
            name: "Weekend plans",
            preview: "Saturday afternoon works",
            initials: "WP",
            color: Color32::from_rgb(210, 99, 91),
            unread: 0,
            online: false,
            pinned: false,
        },
        Conversation {
            name: "Lin",
            preview: "Thanks again!",
            initials: "LI",
            color: Color32::from_rgb(85, 177, 173),
            unread: 0,
            online: false,
            pinned: false,
        },
        Conversation {
            name: "Game night",
            preview: "Rhea: 8pm in the lobby",
            initials: "GN",
            color: Color32::from_rgb(117, 139, 199),
            unread: 1,
            online: true,
            pinned: false,
        },
    ]
}

pub fn messages() -> Vec<Message> {
    vec![
        Message { author: "Juniper", initials: "JU", color: Color32::from_rgb(216, 123, 151), time: "Today at 9:41 AM", body: "Hey! I reorganized the project notes and left a few comments on the new navigation.".into(), mine: false },
        Message { author: "You", initials: "YO", color: Color32::from_rgb(112, 132, 255), time: "Today at 9:45 AM", body: "Nice, thank you. I’m aiming for something that feels calm even when there are a lot of conversations.".into(), mine: true },
        Message { author: "Juniper", initials: "JU", color: Color32::from_rgb(216, 123, 151), time: "Today at 9:46 AM", body: "The clearer spacing definitely helps. The detail panel also gives us a good home for shared files later.".into(), mine: false },
        Message { author: "Juniper", initials: "JU", color: Color32::from_rgb(216, 123, 151), time: "Today at 9:47 AM", body: "Do you want me to turn the component notes into tickets?".into(), mine: false },
        Message { author: "You", initials: "YO", color: Color32::from_rgb(112, 132, 255), time: "Today at 9:52 AM", body: "Yes please — keep them small and backend-agnostic for now. This screen should stay useful as the app evolves.".into(), mine: true },
        Message { author: "Juniper", initials: "JU", color: Color32::from_rgb(216, 123, 151), time: "Today at 9:54 AM", body: "That sounds perfect — ship it.".into(), mine: false },
    ]
}
