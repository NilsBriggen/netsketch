//! Software a device can have, like on a real computer: the apps on its desktop (browser, terminal, files, editor,
//! mail) and the servers it can run (web, DNS, DHCP, mail, file sharing, printing), plus its small file system and
//! mail. Which kind of device may install what, and what it comes with.
use crate::net::{Device, Kind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash, PartialOrd, Ord)]
pub enum Soft {
    Browser,
    Terminal,
    Files,
    Editor,
    MailClient,
    Firewall,
    WebServer,
    DnsServer,
    DhcpServer,
    MailServer,
    FileShare,
    PrintServer,
}

impl Soft {
    pub const ALL: [Soft; 12] = [Soft::Browser, Soft::Terminal, Soft::Files, Soft::Editor, Soft::MailClient, Soft::Firewall, Soft::WebServer, Soft::DnsServer, Soft::DhcpServer, Soft::MailServer, Soft::FileShare, Soft::PrintServer];

    pub fn label(self) -> &'static str {
        match self {
            Soft::Browser => "Web browser",
            Soft::Terminal => "Command line",
            Soft::Files => "Files",
            Soft::Editor => "Text editor",
            Soft::MailClient => "Email",
            Soft::Firewall => "Firewall",
            Soft::WebServer => "Web server",
            Soft::DnsServer => "DNS server",
            Soft::DhcpServer => "DHCP server",
            Soft::MailServer => "Mail server",
            Soft::FileShare => "File sharing",
            Soft::PrintServer => "Print server",
        }
    }

    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as ic;
        match self {
            Soft::Browser => ic::GLOBE,
            Soft::Terminal => ic::TERMINAL_WINDOW,
            Soft::Files => ic::FOLDER_OPEN,
            Soft::Editor => ic::NOTE_PENCIL,
            Soft::MailClient => ic::ENVELOPE_SIMPLE,
            Soft::Firewall => ic::SHIELD_CHECK,
            Soft::WebServer => ic::BROWSERS,
            Soft::DnsServer => ic::ADDRESS_BOOK,
            Soft::DhcpServer => ic::HAND_COINS,
            Soft::MailServer => ic::MAILBOX,
            Soft::FileShare => ic::FOLDER_SIMPLE_USER,
            Soft::PrintServer => ic::PRINTER,
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Soft::Browser => "Opens web pages: on servers in your plan, or real websites through the simulated network.",
            Soft::Terminal => "ping, traceroute, nslookup, ipconfig and more: type help for the list.",
            Soft::Files => "This device's folders and files, and shared folders on other devices.",
            Soft::Editor => "Writes and changes text files, like the web server's pages.",
            Soft::MailClient => "Writes and reads email: sends by SMTP and fetches by IMAP through your mail server.",
            Soft::Firewall => "Lets this device refuse traffic it doesn't want, by rules you write.",
            Soft::WebServer => "Answers browsers (HTTP and HTTPS) with the pages in its webserver folder.",
            Soft::DnsServer => "Turns names into addresses for everyone who asks it.",
            Soft::DhcpServer => "Hands out addresses automatically to the devices in its network.",
            Soft::MailServer => "Takes mail for its mailboxes (SMTP) and lets their owners read it (IMAP).",
            Soft::FileShare => "Shares its 'shared' folder over the network (SMB).",
            Soft::PrintServer => "Takes print jobs over the network (IPP).",
        }
    }

    /// The network service this software runs, if it's a server (the name `Kind::offers` uses).
    pub fn service(self) -> Option<&'static str> {
        match self {
            Soft::WebServer => Some("web"),
            Soft::DnsServer => Some("dns"),
            Soft::DhcpServer => Some("dhcp"),
            Soft::MailServer => Some("mail"),
            Soft::FileShare => Some("files"),
            Soft::PrintServer => Some("print"),
            _ => None,
        }
    }

    pub fn is_server(self) -> bool {
        self.service().is_some()
    }

    /// Whether this kind of device can have it at all.
    pub fn fits(self, k: Kind) -> bool {
        use Kind::*;
        let computer = matches!(k, Desktop | Laptop | Server | Nas);
        match self {
            Soft::Browser => k.has_browser(),
            Soft::Terminal => k.has_shell(),
            Soft::Files | Soft::Editor => computer || k == Phone,
            Soft::MailClient => matches!(k, Desktop | Laptop | Phone | Server),
            Soft::Firewall => computer,
            s => s.service().is_some_and(|svc| k.offers(svc)),
        }
    }

    /// What a new device of this kind comes with.
    pub fn defaults(k: Kind) -> Vec<Soft> {
        use Kind::*;
        let mut v: Vec<Soft> = match k {
            Desktop | Laptop => vec![Soft::Browser, Soft::Terminal, Soft::Files, Soft::Editor, Soft::MailClient],
            Phone => vec![Soft::Browser, Soft::Terminal, Soft::Files, Soft::MailClient],
            Server => vec![Soft::Browser, Soft::Terminal, Soft::Files, Soft::Editor, Soft::WebServer],
            Nas => vec![Soft::Terminal, Soft::Files, Soft::Editor, Soft::FileShare, Soft::WebServer],
            Printer => vec![Soft::WebServer, Soft::PrintServer],
            Router | HomeRouter => vec![Soft::Terminal, Soft::DhcpServer, Soft::DnsServer],
            Internet => vec![Soft::Browser, Soft::Terminal, Soft::DnsServer, Soft::DhcpServer, Soft::WebServer],
            _ => vec![],
        };
        v.retain(|s| s.fits(k));
        v
    }
}

/// Whether a kind of device gets a desktop at all (it can have any software).
pub fn has_desktop(k: Kind) -> bool {
    Soft::ALL.iter().any(|s| s.fits(k))
}

/// One email, kept on the mail server it was delivered to.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Mail {
    pub from: String,
    pub to: String,
    pub subject: String,
    pub body: String,
    pub read: bool,
}

/// A mail program's account: the address it sends as and the mail server (device name) it uses.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MailAccount {
    pub address: String,
    pub server: String,
}

impl Device {
    pub fn has(&self, s: Soft) -> bool {
        self.installed.contains(&s)
    }

    /// Whether a server is installed and started.
    pub fn running(&self, s: Soft) -> bool {
        self.has(s)
            && match s {
                Soft::WebServer => self.services.web,
                Soft::DnsServer => self.services.dns,
                Soft::DhcpServer => self.services.dhcp,
                Soft::MailServer => self.services.mail,
                Soft::FileShare => self.services.files,
                Soft::PrintServer => self.services.print,
                Soft::Firewall => self.services.firewall,
                _ => true,
            }
    }

    /// Starts or stops a server (and the firewall).
    pub fn set_running(&mut self, s: Soft, on: bool) {
        let on = on && self.has(s);
        match s {
            Soft::WebServer => self.services.web = on,
            Soft::DnsServer => self.services.dns = on,
            Soft::DhcpServer => self.services.dhcp = on,
            Soft::MailServer => self.services.mail = on,
            Soft::FileShare => self.services.files = on,
            Soft::PrintServer => self.services.print = on,
            Soft::Firewall => self.services.firewall = on,
            _ => {}
        }
    }

    pub fn install(&mut self, s: Soft) {
        if s.fits(self.kind) && !self.has(s) {
            self.installed.push(s);
            self.installed.sort();
            // A fresh firewall comes with a note on how its rules work; servers start straight away.
            if s == Soft::Firewall && self.rules.trim().is_empty() {
                self.rules = "# First matching line wins; anything no line matches is let through.\n# e.g. deny any any 22    or    allow 192.168.1.0/24 any\n".into();
            }
            if s.is_server() || s == Soft::Firewall {
                self.set_running(s, true);
            }
            if s == Soft::WebServer && self.file("webserver/index.html").is_none() {
                let page = format!("<html><body><h1>{}</h1><p>This page lives in the webserver folder: change it with the text editor.</p></body></html>", self.name);
                self.set_file("webserver/index.html", &page);
            }
        }
    }

    pub fn uninstall(&mut self, s: Soft) {
        self.set_running(s, false);
        self.installed.retain(|x| *x != s);
    }

    /// A file's text, by its path ("webserver/index.html").
    pub fn file(&self, path: &str) -> Option<&str> {
        let p = clean(path);
        self.files.iter().find(|f| f.0.eq_ignore_ascii_case(&p)).map(|f| f.1.as_str())
    }

    pub fn set_file(&mut self, path: &str, text: &str) {
        let p = clean(path);
        match self.files.iter_mut().find(|f| f.0.eq_ignore_ascii_case(&p)) {
            Some(f) => f.1 = text.into(),
            None => self.files.push((p, text.into())),
        }
        self.files.sort_by(|a, b| a.0.cmp(&b.0));
    }

    /// The files right inside a folder ("" = the top), with their names.
    pub fn files_in(&self, dir: &str) -> Vec<(String, String)> {
        let d = clean(dir);
        let pre = if d.is_empty() { String::new() } else { format!("{d}/") };
        self.files.iter().filter(|f| f.0.starts_with(&pre) && !f.0[pre.len()..].contains('/') && !f.0.ends_with("/.keep")).map(|f| (f.0[pre.len()..].to_string(), f.0.clone())).collect()
    }

    /// The folders right inside a folder.
    pub fn dirs_in(&self, dir: &str) -> Vec<String> {
        let d = clean(dir);
        let pre = if d.is_empty() { String::new() } else { format!("{d}/") };
        let mut v: Vec<String> = self.files.iter().filter_map(|f| f.0.strip_prefix(&pre).and_then(|r| r.split_once('/')).map(|x| x.0.to_string())).collect();
        v.sort();
        v.dedup();
        v
    }
}

/// A path without leading or doubled slashes.
pub fn clean(p: &str) -> String {
    p.split('/').filter(|s| !s.is_empty() && *s != ".").collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_get_fitting_software_and_files_live_in_folders() {
        assert!(Soft::defaults(Kind::Laptop).contains(&Soft::Browser) && !Soft::WebServer.fits(Kind::Phone));
        assert!(Soft::MailServer.fits(Kind::Server) && !has_desktop(Kind::Switch) && has_desktop(Kind::Printer));
        let mut d = Device { kind: Kind::Server, ..Default::default() };
        d.install(Soft::WebServer);
        assert!(d.running(Soft::WebServer) && d.file("/webserver//index.html").is_some());
        d.set_file("webserver/about.html", "about");
        d.set_file("notes.txt", "hi");
        assert_eq!(d.dirs_in(""), vec!["webserver".to_string()]);
        assert_eq!(d.files_in("webserver").len(), 2);
        d.uninstall(Soft::WebServer);
        assert!(!d.services.web && !d.has(Soft::WebServer));
    }
}
