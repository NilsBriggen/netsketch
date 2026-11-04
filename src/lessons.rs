//! Short, up-to-date lessons shown in Learn mode: (title, body, try-it hint, the scenario key it uses or "").
pub const LESSONS: &[(&str, &str, &str, &str)] = &[
    (
        "How a network fits together",
        "Devices talk in layers. The link layer (Ethernet or Wi-Fi) moves frames between neighbours using MAC addresses. The network layer (IP) carries packets across networks, hop by hop, through routers. The transport layer (TCP or UDP) uses port numbers so many apps can share one address. On top sit applications such as the web (HTTPS), DNS and file sharing.\n\nA switch connects devices inside one network. A router connects networks to each other — at home, the internet provider's network and yours. The old \"hub\", which copied everything to every port, is obsolete: switches cost the same and are faster and more private.",
        "Open the template \"Two networks and a router\", switch to Simulate and run traceroute web-server on PC 1.",
        "classroom",
    ),
    (
        "IPv4 addresses and subnets",
        "An IPv4 address has four numbers (192.168.1.20). The prefix — /24, written as the mask 255.255.255.0 in older tools — says which part names the network: with /24, all addresses 192.168.1.x are neighbours and reach each other directly. Anything else goes to the default gateway (usually your router).\n\nPrivate ranges, usable in any home or office: 10.0.0.0/8, 172.16.0.0/12 and 192.168.0.0/16. They are never routed on the public internet. 169.254.x.x means a device asked for an address and nobody answered. This app uses the documentation ranges 198.51.100.0/24 and 203.0.113.0/24 for its make-believe internet.",
        "Give a PC a fixed address in a different network and watch the problem list.",
        "",
    ),
    (
        "IPv6 is here",
        "The world ran out of new IPv4 addresses: the central pool was used up in 2011 and the regional registries followed (Europe in 2019). IPv6 fixes this with 128-bit addresses such as 2001:db8:12:34::20. Today close to half of the traffic reaching large websites uses IPv6, and most fibre, cable and mobile providers hand it out by default.\n\nWith IPv6 a home usually receives a whole /56 or /64 prefix; devices build their own addresses (SLAAC) and every device can be globally reachable, so the router's firewall — not NAT — keeps unwanted traffic out. Neighbour Discovery replaces ARP. Most homes run both versions side by side (\"dual stack\").",
        "Run ipconfig in Simulate mode: each device also shows its fe80:: link-local IPv6 address.",
        "",
    ),
    (
        "DHCP: automatic addresses",
        "When a device joins, it shouts DISCOVER. A DHCP server (normally the home router) answers with an OFFER: an address, the prefix, the gateway and the DNS server. The device REQUESTs it and the server ACKnowledges — the four-step \"DORA\" exchange. Addresses are leased for a while and renewed automatically.\n\nPlanning tip: let phones and laptops use DHCP, and give always-on devices such as a NAS or printer a fixed address outside the DHCP pool (or a reservation in the router), so they never move. Only one DHCP server should run per network.",
        "Start Simulate: the packet log shows DISCOVER, OFFER, REQUEST and ACK for every device.",
        "",
    ),
    (
        "DNS: names to addresses",
        "People type names, computers need addresses. A DNS resolver looks names up for you, asking the root, then the .com servers, then the domain's own server, and caches the answer. Your home router usually forwards questions to the provider's resolver; many people pick a public one instead.\n\nModern resolvers can be reached encrypted with DNS over HTTPS (DoH) or DNS over TLS (DoT), which browsers and phones support out of the box. Inside a home, routers often answer for local devices; the official name for home networks is .home.arpa (RFC 8375) — so nas-1.home.arpa works.",
        "On any device in the home template run nslookup nas-1 and then nslookup example.com.",
        "home",
    ),
    (
        "NAT and the home router",
        "Your router has one public IPv4 address from the provider. When a device inside sends a packet out, the router swaps the private sender address for its public one and remembers the conversation (Network Address Translation). Replies are translated back. Unsolicited packets from outside find no entry and are dropped — that's why you can't reach a home device from the internet without port forwarding.\n\nMany providers now share one public IPv4 address among many customers (Carrier-grade NAT, range 100.64.0.0/10), which makes port forwarding impossible; IPv6 or a VPN/tunnel service is the modern way to reach home devices from outside.",
        "Ping example.com and look for the NAT lines in the packet log.",
        "",
    ),
    (
        "Wi-Fi today",
        "Wi-Fi generations now have simple names: Wi-Fi 4 (802.11n), 5 (802.11ac), 6 (802.11ax), 6E (adds the 6 GHz band) and Wi-Fi 7 (802.11be, certified in 2024) with 320 MHz channels and Multi-Link Operation that uses several bands at once. Wi-Fi 8 is being developed with a focus on reliability.\n\n2.4 GHz reaches far but is slow and crowded; 5 GHz is the everyday band; 6 GHz is fast and clean but is stopped by walls more easily. One strong router rarely covers a whole house: add access points connected by cable (best) or a mesh system. Real speeds are well below the numbers on the box.",
        "Turn on the Wi-Fi map (the Wi-Fi button in the top bar) and move a laptop away from the router.",
        "",
    ),
    (
        "Wi-Fi security",
        "WEP was broken in 2001 and must not be used. WPA2 (2004) is still common but weak passwords can be guessed offline after capturing one login. WPA3 (2018) uses SAE, which stops offline guessing, and protected management frames; it is mandatory on 6 GHz. Use WPA3, or the WPA2/WPA3 transition mode if old devices must join.\n\nAlso: change the router's admin password, keep its firmware updated, and turn off WPS. Put guests and smart home gadgets on a separate guest or IoT network so a weak device can't reach your laptop or NAS.",
        "Set the router's security to WEP and watch the problem list.",
        "",
    ),
    (
        "Cables and wired speed",
        "Copper Ethernet runs up to 100 m per cable. Cat 5e carries 1 Gbit/s and usually 2.5 Gbit/s; Cat 6 carries 10 Gbit/s up to about 55 m; Cat 6A carries 10 Gbit/s over the full 100 m and is the sensible choice for new installations. Fibre goes much further and faster.\n\n2.5 Gbit/s ports are now common on routers, PCs and NAS boxes, and fibre internet plans above 1 Gbit/s are widespread. Power over Ethernet (PoE, 802.3af/at/bt) powers access points and cameras through the same cable. A cable is still the most reliable link for a desktop, TV or NAS.",
        "Select a cable to see its type, length and speed. Lengths come from the drawing — the dot spacing is shown next to the zoom level.",
        "",
    ),
    (
        "The web and HTTPS",
        "Almost every web page is now loaded over HTTPS: the browser and server do a TLS handshake (TLS 1.3 is current) and everything after it is encrypted and checked. Certificates are free and renewed automatically. HTTP/2 sends many requests over one connection; HTTP/3 (2022) runs over QUIC on UDP and is used by most large sites.\n\nPlain HTTP, FTP and Telnet send everything readable on the wire — use HTTPS, SFTP and SSH instead. Email is sent with SMTP (port 587 with STARTTLS or 465) and read with IMAP over TLS (993); POP3 is rarely used anymore.",
        "Run open example.com and read the TCP, TLS and HTTP lines in the packet log.",
        "",
    ),
    (
        "Smart home and storage",
        "Smart home devices increasingly use Matter (released 2022), a shared standard that runs over Wi-Fi, Ethernet or Thread — a low-power mesh that needs a \"border router\", often built into a smart speaker or hub. Matter devices work locally, without a cloud account.\n\nA NAS shares files with SMB 3 (old SMB 1 is insecure and switched off in current systems). Follow the 3-2-1 rule: three copies of important data, on two kinds of media, one off-site — a NAS alone is not a backup.",
        "Run test nas-1 files from the phone in the home template.",
        "home",
    ),
    (
        "Walls, distance and signal strength",
        "Wi-Fi signal is measured in dBm: −30 is right next to the router, −55 is excellent, −67 is what video calls want, −75 is where things get flaky and below −85 nothing works. Every 3 dB is half the power; every 10 dB is a tenth.\n\nDistance costs signal, and walls cost more: drywall a few dB, brick around 10, reinforced concrete 15–25, and energy-saving (low-E) windows or metal 25–35. 5 and 6 GHz are faster but lose more per wall than 2.4 GHz. Metal also reflects: it leaves a dead zone behind it and echoes in front of it. That's why routers in a cupboard, behind a TV or next to the fridge disappoint.",
        "Open 'Typical home', turn on the Wi-Fi map and draw a concrete wall through the living room with the Walls tool.",
        "home",
    ),
    (
        "Power, antennas, bands and channel width",
        "A router's transmit power is legally capped (in the EU about 20 dBm on 2.4 GHz and 23 dBm on 5/6 GHz). Turning it up rarely helps: a phone sends with only about 15 dBm, so the router may be heard where the phone's answer never arrives.\n\nSpeed comes from the standard (Wi-Fi 5, 6, 6E, 7), the number of antenna streams (2×2 in most phones, 4×4 in good routers) and the channel width: 20, 40, 80, 160 or — with Wi-Fi 7 on 6 GHz — 320 MHz. Wider channels are faster up close but pick up more noise and collide with neighbours more easily. Band steering moves capable devices off crowded 2.4 GHz.",
        "Select the router, pick a different hardware preset and watch the speeds on the map and in the problem list change.",
        "",
    ),
    (
        "Neighbours and channels",
        "Wi-Fi is polite: on one channel only one device talks at a time. If your neighbours use the same channel, you take turns with them — your speed drops even though your signal is great. On 2.4 GHz only channels 1, 6 and 11 don't overlap; the ones in between are worse because they collide with two others at once.\n\n5 GHz has many more channels, some of them shared with weather radar (DFS): the router must listen first and may jump away. 6 GHz is wide open and stops at walls — so it rarely meets the neighbours. Pick channels after looking at what's in the air.",
        "Open 'Crowded apartment block', run wifi on the laptop, then press 'Pick better channels'.",
        "apartment",
    ),
    (
        "Range extenders, mesh and roaming",
        "A plug-in range extender listens to the router and repeats everything on the same radio: every packet crosses the air twice, so speed is at best halved — and if it's placed where the router is already weak, it can only repeat a weak signal. Extenders with their own network name ('…_EXT') are worse: devices never switch between the two names by themselves.\n\nBetter: an access point on a cable, or a mesh system with a separate backhaul radio. Phones decide themselves when to switch access points ('roaming'). Many hang on far too long (sticky clients). 802.11k and v help them choose well and in time, 802.11r makes the switch take ~40 ms instead of a noticeable break.",
        "Open 'Walk & roam', switch to Simulate, select the phone and press 'Take a walk'. Then turn on fast roaming and walk again.",
        "roaming",
    ),
    (
        "Spanning tree — and the speaker that ate the network",
        "Two cables between the same switches form a loop: broadcasts circle forever and the network collapses (a broadcast storm). Spanning tree (STP/RSTP) prevents this. The switches elect a root bridge — lowest priority wins, then the lowest MAC address — and every other switch keeps only its best path to the root; the rest go on standby.\n\nSome multi-room speakers build their own wireless mesh and also run spanning tree. Wire two of them to different switches and they become part of your switch network. Everything ships with priority 32768, so the lowest MAC address decides — and that depends on the hardware you happen to own. Some speakers also count costs on the old short scale (1 Gbit/s = 4 instead of 20000) behind slow 100 Mbit ports, so to your modern switches the path through them looks cheap. If a speaker wins the election, your fast cable can go on standby while everything crawls through the speakers' 2.4 GHz mesh. Fix: give your main switch priority 4096, or wire only one speaker.",
        "Open 'The speaker that ate the network', run speedtest and stp on the Office PC, then press 'Make it the root'.",
        "speakers",
    ),
    (
        "VLANs: many networks on one cable",
        "A VLAN splits one physical network into several separate ones. Each frame on a shared (trunk) cable carries a tag with its VLAN number (802.1Q, 1–4094). Devices in different VLANs can't see each other's broadcasts; traffic between VLANs must pass a router or firewall, where rules can apply.\n\nOffices separate staff, guests, phones, cameras and printers this way, often with dozens or hundreds of VLANs. At home, a guest or IoT VLAN keeps cheap gadgets away from your laptop and NAS. Every VLAN needs its own router interface and DHCP pool — a device in a VLAN nobody routes gets a 169.254 address and stays stranded.",
        "Open 'Office with 100 VLANs' and find out why the mislabelled PC can't reach anything.",
        "office",
    ),
    (
        "From plan to the real building",
        "A plan is only as good as what it's based on. Put a picture of each storey's floor plan underneath, set its real width, and trace the walls with their materials. Then check reality: walk around with a Wi-Fi app, note the signal at a few spots and import or record those readings. Calibration shifts each access point's model so the average error drops — usually to a few dB.\n\nDon't forget power and load. Access points and cameras are often powered over the cable (PoE): the switch must supply the right class (PoE+ gives 30 W per port) and its total budget must cover everything. And plan for the busy hour — streaming, video calls, backups and game downloads at once. A link that's over 80% full adds delay, which video calls and games feel first.",
        "Open 'Typical home', tick 'Big downloads' on a few devices, then switch to Simulate and ping from the Office PC.",
        "home",
    ),
    (
        "When things break — and other real-world setups",
        "Every network has weak spots: the one switch or router whose failure cuts everyone off. Find them before they fail — switch a device off or unplug a cable in Simulate and see what still works. Common cures: a second cable path (spanning tree keeps it on standby until it's needed), a spare switch, or a mobile backup line for the internet.\n\nPlugging a second router's internet port into your network creates a network behind a network: addresses are translated twice (double NAT). It works for browsing, but port forwarding, some games and calls struggle — usually you want the second box in access-point mode. And one access point can broadcast several names at once: home, guest and gadgets, each leading into its own VLAN, so a visitor's phone never sees your NAS.",
        "In 'Typical home', switch the Home router off in Simulate, then look at 'Weak spots' in the overview.",
        "home",
    ),
];

#[cfg(test)]
mod tests {
    #[test]
    fn every_lesson_names_a_real_scenario_and_mentions_it() {
        for (title, _, try_it, key) in super::LESSONS {
            if key.is_empty() {
                continue;
            }
            let s = crate::scenarios::SCENARIOS.iter().find(|s| s.0 == *key).unwrap_or_else(|| panic!("{title}: no scenario '{key}'"));
            let name = s.1.split(" (").next().unwrap();
            assert!(try_it.contains(name) || try_it.contains(&format!("{key} template")), "{title}: the hint doesn't say '{name}'");
        }
    }
}
