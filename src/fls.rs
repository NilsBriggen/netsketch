//! Import of older .fls projects: a zip holding projekt/konfiguration.xml in Java XMLEncoder format
//! (a version string, then a list of device items, a list of cable items and a list of notes).
//! Layout checked against a real saved project (Filius 2.4.1).
use crate::net::{parse_ip, Kind, Medium, Network};
use roxmltree::{Document, Node};
use std::collections::HashMap;
use std::io::Read;

/// Returns the plan and how many cables couldn't be brought over.
pub fn import(bytes: &[u8]) -> Result<(Network, usize), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "This isn't a valid .fls project (not a zip archive).")?;
    let mut xml = String::new();
    // A real project is a few hundred KB; the cap keeps a zip bomb from filling the memory.
    zip.by_name("projekt/konfiguration.xml")
        .map_err(|_| "This .fls file has no project configuration inside.".to_string())?
        .take(50 << 20)
        .read_to_string(&mut xml)
        .map_err(|e| format!("Couldn't unpack the project: {e}"))?;
    from_xml(&xml)
}

/// The `<object>` elements added to an XMLEncoder list.
fn list_items<'a>(list: Node<'a, 'a>) -> Vec<Node<'a, 'a>> {
    list.children()
        .filter(|v| v.attribute("method") == Some("add"))
        .filter_map(|v| v.children().find(|o| o.has_tag_name("object")))
        .collect()
}

/// Text of the first `<void property=…>` under `n` whose name matches.
fn prop(n: Node, matches: impl Fn(&str) -> bool) -> Option<String> {
    n.descendants()
        .filter(|v| v.has_tag_name("void") && v.attribute("property").is_some_and(|p| matches(&p.to_lowercase())))
        .find_map(|v| v.children().find(|c| c.is_element()).and_then(|c| c.text()).map(|t| t.trim().to_string()))
        .filter(|t| !t.is_empty())
}

fn class_is(n: &Node, suffix: &str) -> bool {
    n.attribute("class").is_some_and(|c| c.ends_with(suffix))
}

fn prefix_of(mask: Option<String>) -> u8 {
    mask.and_then(|m| parse_ip(&m)).map(|m| u32::from(m).count_ones() as u8).unwrap_or(24)
}

/// A field of the java.awt.Rectangle stored as the device's on-screen bounds.
fn rect_field(n: Node, name: &str) -> Option<f32> {
    n.descendants()
        .filter(|v| v.attribute("method") == Some("getField"))
        .find(|v| v.children().any(|c| c.has_tag_name("string") && c.text() == Some(name)))?
        .descendants()
        .find(|c| c.has_tag_name("int"))?
        .text()?
        .trim()
        .parse()
        .ok()
        .filter(|v: &f32| v.is_finite() && v.abs() < 1e6)
}

pub fn from_xml(xml: &str) -> Result<(Network, usize), String> {
    let doc = Document::parse(xml).map_err(|e| format!("The project file is damaged: {e}"))?;
    let lists: Vec<Node> = doc.root_element().children().filter(|n| n.has_tag_name("object")).collect();
    // Saved projects list devices first, then cables, then notes.
    let devices = lists.first().map(|l| list_items(*l)).unwrap_or_default();
    let cables = lists.get(1).map(|l| list_items(*l)).unwrap_or_default();

    let mut net = Network { title: "Imported project".into(), currency: "€".into(), ..Default::default() };
    // XML id -> (device index, interface index inside that device).
    let mut ids: HashMap<&str, (usize, Option<usize>)> = HashMap::new();
    let mut new_ids = vec![];
    let mut router_ifaces: Vec<Vec<(String, u8)>> = vec![];
    for (i, item) in devices.iter().enumerate() {
        let node = item.descendants().find(|n| n.attribute("class").is_some_and(|c| c.starts_with("filius.hardware.knoten."))).unwrap_or(*item);
        let src = &xml[item.range()];
        let kind = match node.attribute("class").unwrap_or("").rsplit('.').next().unwrap_or("") {
            "Notebook" => Kind::Laptop,
            "Switch" | "Modem" => Kind::Switch,
            "Vermittlungsrechner" => Kind::Router,
            _ if src.contains("filius.software.www.WebServer") || src.contains("DNSServer\"") => Kind::Server,
            _ => Kind::Desktop,
        };
        // Interfaces are written as <void id="NetzwerkInterfaceN" index=…> (or a full object).
        let ifaces: Vec<Node> = node
            .descendants()
            .filter(|n| n.attribute("id").is_some_and(|i| i.starts_with("NetzwerkInterface")) || class_is(n, "NetzwerkInterface"))
            .collect();
        for n in item.descendants() {
            if let Some(id) = n.attribute("id") {
                let iface = ifaces.iter().position(|f| n.ancestors().any(|a| a == *f));
                ids.insert(id, (i, iface));
            }
        }
        let pos = match (rect_field(*item, "x"), rect_field(*item, "y")) {
            (Some(x), Some(y)) => [x + rect_field(*item, "width").unwrap_or(0.0) / 2.0, y + rect_field(*item, "height").unwrap_or(0.0) * 0.4],
            _ => [120.0 + (i % 6) as f32 * 160.0, 100.0 + (i / 6) as f32 * 150.0],
        };
        let id = net.add_device(kind, pos);
        new_ids.push(id);
        let addrs: Vec<(String, u8)> = ifaces
            .iter()
            .map(|f| (prop(*f, |p| p == "ip" || p == "ipadresse").unwrap_or_default(), prefix_of(prop(*f, |p| p.contains("mask")))))
            .collect();
        let mac = ifaces.first().and_then(|f| prop(*f, |p| p == "mac"));
        let d = net.dev_mut(id).unwrap();
        if let Some(name) = prop(*item, |p| p == "text").or_else(|| prop(node, |p| p == "name")) {
            d.name = name;
        }
        if kind.is_host() {
            if let Some((ip, p)) = addrs.first().filter(|a| !a.0.is_empty()) {
                d.ifaces[0].ip = ip.clone();
                d.ifaces[0].prefix = *p;
                d.dhcp_client = false;
            }
            if let Some(mac) = mac {
                d.ifaces[0].mac = mac.to_lowercase();
            }
            d.gateway = prop(node, |p| p.contains("gateway")).unwrap_or_default();
            d.dns = prop(node, |p| p.contains("dns")).filter(|v| parse_ip(v).is_some()).unwrap_or_default();
            d.services.web = src.contains("filius.software.www.WebServer");
            d.services.dns = src.contains("DNSServer\"");
            // Web pages the old project stored in the device's file system.
            for f in node.descendants().filter(|n| class_is(n, "system.Datei")) {
                let name = prop(f, |p| p == "name").unwrap_or_default();
                if name.ends_with(".html") || name.ends_with(".htm") {
                    let body = prop(f, |p| p == "dateiinhalt").unwrap_or_default();
                    if name.eq_ignore_ascii_case("index.html") {
                        d.web_page = body;
                    } else {
                        d.web_files.push((name, body));
                    }
                }
            }
        }
        router_ifaces.push(addrs);
    }

    let mut skipped = 0;
    let mut used: Vec<usize> = vec![0; devices.len()];
    for c in &cables {
        let mut ends: Vec<(usize, Option<usize>)> = vec![];
        for r in c.descendants().filter_map(|n| n.attribute("idref")).filter_map(|r| ids.get(r)) {
            if !ends.iter().any(|e| e.0 == r.0) {
                ends.push(*r);
            }
        }
        let [a, b] = ends[..] else {
            skipped += 1;
            continue;
        };
        let wireless = prop(*c, |p| p == "wireless").as_deref() == Some("true");
        let Ok(link) = net.connect_unplugged(new_ids[a.0], new_ids[b.0], if wireless { Medium::WiFi } else { Medium::Cat6 }) else {
            skipped += 1;
            continue;
        };
        let l = net.links.iter().find(|l| l.id == link).unwrap().clone();
        for ((dev, iface), port) in [(a, l.a_if), (b, l.b_if)] {
            let d = net.dev_mut(new_ids[dev]).unwrap();
            // The old "Switch / Wi-Fi" box: once something joins it by radio it is an access point here.
            if wireless && d.kind == Kind::Switch {
                d.kind = Kind::AccessPoint;
                d.price = Kind::AccessPoint.price();
            }
            if d.kind != Kind::Router {
                continue;
            }
            // Routers got a fresh port for this cable: give it the address the old project had there.
            let k = iface.unwrap_or(used[dev]);
            used[dev] += 1;
            if let Some((ip, p)) = router_ifaces[dev].get(k) {
                d.ifaces[port].ip = ip.clone();
                d.ifaces[port].prefix = *p;
            }
        }
    }
    net.assign_ports();
    Ok((net, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Trimmed from a real saved project: two notebooks (one on Wi-Fi), a web server and a switch.
    const SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<java version="17.0.7" class="java.beans.XMLDecoder">
 <string>Filius version: 2.4.1 (07.06.2023)</string>
 <object class="java.util.LinkedList">
  <void method="add">
   <object class="filius.gui.netzwerksicht.GUIKnotenItem" id="GUIKnotenItem0">
    <void property="imageLabel"><object class="filius.gui.netzwerksicht.JSidebarButton">
     <void property="bounds"><object class="java.awt.Rectangle" id="Rectangle0">
      <void class="java.awt.Rectangle" method="getField"><string>x</string><void method="set"><object idref="Rectangle0"/><int>145</int></void></void>
      <void class="java.awt.Rectangle" method="getField"><string>y</string><void method="set"><object idref="Rectangle0"/><int>366</int></void></void>
      <void class="java.awt.Rectangle" method="getField"><string>width</string><void method="set"><object idref="Rectangle0"/><int>100</int></void></void>
      <void class="java.awt.Rectangle" method="getField"><string>height</string><void method="set"><object idref="Rectangle0"/><int>100</int></void></void>
     </object></void>
     <void property="text"><string>Notebook 1</string></void>
    </object></void>
    <void property="knoten"><object class="filius.hardware.knoten.Notebook" id="Notebook0">
     <void property="name"><string>Notebook 1</string></void>
     <void id="LinkedList0" property="netzwerkInterfaces"><void id="NetzwerkInterface0" index="0">
      <void property="ip"><string>192.168.1.110</string></void>
      <void property="mac"><string>19:03:C4:EF:C8:BE</string></void>
      <void id="Port0" property="port"/>
     </void></void>
     <void id="Betriebssystem0" property="systemSoftware"><void property="DHCPServer"><void property="name"><string>Thread-1</string></void></void>
      <void property="installierteAnwendungen"><void method="put"><string>filius.software.www.WebBrowser</string><object class="filius.software.www.WebBrowser"/></void></void>
     </void>
    </object></void>
   </object>
  </void>
  <void method="add">
   <object class="filius.gui.netzwerksicht.GUIKnotenItem" id="GUIKnotenItem1">
    <void property="imageLabel"><object class="filius.gui.netzwerksicht.JSidebarButton"><void property="text"><string>Serveur Web</string></void></object></void>
    <void property="knoten"><object class="filius.hardware.knoten.Rechner" id="Rechner0">
     <void property="name"><string>Serveur Web</string></void>
     <void id="LinkedList1" property="netzwerkInterfaces"><void id="NetzwerkInterface1" index="0">
      <void property="ip"><string>192.168.1.10</string></void>
      <void id="Port1" property="port"/>
     </void></void>
     <void id="Betriebssystem1" property="systemSoftware">
      <void id="Dateisystem1" property="dateisystem"><void id="DefaultMutableTreeNode1" property="arbeitsVerzeichnis"><void method="add"><object class="javax.swing.tree.DefaultMutableTreeNode">
       <void property="userObject"><string>webserver</string></void>
       <void method="add"><object class="javax.swing.tree.DefaultMutableTreeNode"><void property="userObject"><object class="filius.software.system.Datei">
        <void property="dateiInhalt"><string>&lt;html&gt;&lt;h1&gt;Hello&lt;/h1&gt;&lt;/html&gt;</string></void>
        <void property="dateiTyp"><string>html</string></void>
        <void property="name"><string>index.html</string></void>
       </object></void></object></void>
      </object></void></void></void>
      <void property="installierteAnwendungen"><void method="put"><string>filius.software.www.WebServer</string><object class="filius.software.www.WebServer"/></void></void>
     </void>
    </object></void>
   </object>
  </void>
  <void method="add">
   <object class="filius.gui.netzwerksicht.GUIKnotenItem" id="GUIKnotenItem2">
    <void property="imageLabel"><object class="filius.gui.netzwerksicht.JSidebarButton"><void property="text"><string>Switch / WiFi</string></void></object></void>
    <void property="knoten"><object class="filius.hardware.knoten.Switch" id="Switch0">
     <void id="LinkedList2" property="anschluesse"><void id="Port3" index="0"/><void id="Port4" index="1"/></void>
    </object></void>
   </object>
  </void>
 </object>
 <object class="java.util.LinkedList">
  <void method="add"><object class="filius.gui.netzwerksicht.GUIKabelItem" id="GUIKabelItem0">
   <void property="dasKabel"><object class="filius.hardware.Kabel">
    <void property="anschluesse"><array class="filius.hardware.Port" length="2"><void index="0"><object idref="Port0"/></void><void index="1"><object idref="Port3"/></void></array></void>
    <void property="wireless"><boolean>true</boolean></void>
   </object></void>
   <void property="kabelpanel"><void property="ziel1"><object idref="GUIKnotenItem0"/></void><void property="ziel2"><object idref="GUIKnotenItem2"/></void></void>
  </object></void>
  <void method="add"><object class="filius.gui.netzwerksicht.GUIKabelItem" id="GUIKabelItem1">
   <void property="dasKabel"><object class="filius.hardware.Kabel">
    <void property="anschluesse"><array class="filius.hardware.Port" length="2"><void index="0"><object idref="Port1"/></void><void index="1"><object idref="Port4"/></void></array></void>
   </object></void>
  </object></void>
 </object>
 <object class="java.util.ArrayList"/>
</java>"#;

    #[test]
    fn imports_devices_addresses_and_cables() {
        let (mut n, skipped) = from_xml(SAMPLE).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(n.devices.len(), 3);
        let nb = &n.devices[0];
        assert_eq!((nb.name.as_str(), nb.kind, nb.ifaces[0].ip.as_str(), nb.ifaces[0].prefix), ("Notebook 1", Kind::Laptop, "192.168.1.110", 24));
        assert_eq!((nb.pos, nb.ifaces[0].mac.as_str(), nb.dhcp_client), ([195.0, 406.0], "19:03:c4:ef:c8:be", false));
        let srv = &n.devices[1];
        assert_eq!((srv.kind, srv.services.web, srv.web_page.as_str()), (Kind::Server, true, "<html><h1>Hello</h1></html>"));
        assert_eq!(n.devices[2].kind, Kind::AccessPoint);
        assert_eq!(n.links.iter().map(|l| l.medium).collect::<Vec<_>>(), [Medium::WiFi, Medium::Cat6]);
        let id = nb.id;
        let out = n.exec(id, "open 192.168.1.10").lines.join("\n");
        assert!(out.contains("HELLO"), "{out}");
    }

    #[test]
    fn reads_zip_and_empty_project() {
        let xml = r#"<?xml version="1.0"?><java class="java.beans.XMLDecoder"><string>Filius</string><object class="java.util.LinkedList"/><object class="java.util.LinkedList"/><object class="java.util.ArrayList"/></java>"#;
        let mut buf = std::io::Cursor::new(vec![]);
        let mut z = zip::ZipWriter::new(&mut buf);
        z.start_file("projekt/konfiguration.xml", zip::write::SimpleFileOptions::default()).unwrap();
        z.write_all(xml.as_bytes()).unwrap();
        z.finish().unwrap();
        let (n, _) = import(buf.get_ref()).unwrap();
        assert!(n.devices.is_empty());
        assert!(import(b"not a zip").is_err());
    }
}
