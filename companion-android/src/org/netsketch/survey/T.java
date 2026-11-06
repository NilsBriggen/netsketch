package org.netsketch.survey;

import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/**
 * Interface text in the phone's language, like NetSketch on the computer: the English text is the key,
 * German comes from the table below. Plain Java, so test/SurveyJsonTest.java can check that every German
 * text keeps the same placeholders.
 */
public final class T {
    /** Set once at start from the phone's language. */
    public static boolean german = Locale.getDefault().getLanguage().equals("de");

    private T() {}

    public static String t(String en) {
        if (!german) return en;
        String de = DE.get(en);
        return de == null ? en : de;
    }

    /** A translated template with %s / %d / %.0f placeholders, filled in. */
    public static String f(String en, Object... args) {
        // German numbers the way the phone writes them ("3,2 m"; Swiss German keeps "3.2").
        return String.format(german ? Locale.getDefault() : Locale.ROOT, t(en), args);
    }

    public static final Map<String, String> DE = new HashMap<>();

    private static void de(String en, String de) {
        DE.put(en, de);
    }

    static {
        // Header, live signal
        de("Your real Wi-Fi, spot by spot", "Ihr echtes WLAN, Punkt für Punkt");
        de("Link", "Verbinden");
        de("Linking…", "Verbinde…");
        de("Not reachable", "Nicht erreichbar");
        de("Linked · %s", "Verbunden · %s");
        de("Your Wi-Fi right here", "Ihr WLAN genau hier");
        de("Not on Wi-Fi", "Nicht im WLAN");
        de("Connect to your Wi-Fi to see its strength here.", "Verbinden Sie sich mit Ihrem WLAN, um hier seine Stärke zu sehen.");
        de("name hidden until you allow nearby networks", "Name verborgen, bis Sie Netzwerke in der Nähe erlauben");
        de("dBm · %s", "dBm · %s");
        de("%s · %s ch %d · %.0f Mbit/s link", "%s · %s Kanal %d · %.0f Mbit/s Verbindung");
        de("last minute: %.0f to %.0f dBm", "letzte Minute: %.0f bis %.0f dBm");
        de("excellent", "ausgezeichnet");
        de("good", "gut");
        de("fair", "mittel");
        de("weak", "schwach");
        de("very weak", "sehr schwach");
        de("Signal: %s", "Signal: %s");
        // Plan
        de("Where are you?", "Wo sind Sie?");
        de("Link to NetSketch on your computer to tap your spot on the floor plan.", "Verbinden Sie sich mit NetSketch auf dem Computer, um Ihren Standort im Grundriss anzutippen.");
        de("Link to NetSketch", "Mit NetSketch verbinden");
        de("Spot picked: %.1f m, %.1f m — measure when you're standing there.", "Punkt gewählt: %.1f m, %.1f m — messen Sie, wenn Sie dort stehen.");
        de("Pick the storey, then tap where you're standing.", "Wählen Sie das Stockwerk und tippen Sie, wo Sie stehen.");
        de("Tap where you're standing.", "Tippen Sie, wo Sie stehen.");
        de("Tap where you're standing", "Tippen Sie, wo Sie stehen");
        de("Pinch to zoom · double-tap to fit", "Zum Zoomen zwei Finger · doppelt tippen zum Einpassen");
        de("Floor plan. %s", "Grundriss. %s");
        de("Coloured dots: your Wi-Fi measured there", "Farbige Punkte: dort gemessenes WLAN");
        // Measure
        de("Measure", "Messen");
        de("Name this spot, e.g. Sofa", "Name für diesen Punkt, z. B. Sofa");
        de("Storey  ", "Stockwerk  ");
        de("   0 = ground, −1 = basement", "   0 = Erdgeschoss, −1 = Keller");
        de("Speed test too (uses about 35 MB)", "Auch Geschwindigkeit testen (braucht etwa 35 MB)");
        de("Measure here", "Hier messen");
        de("Stand still for about 10 seconds while it listens.", "Bleiben Sie etwa 10 Sekunden stehen, während es lauscht.");
        de("Listening… %d of %d", "Lausche… %d von %d");
        de("Stand still.", "Bitte stillstehen.");
        de("Android allows 4 Wi-Fi scans every 2 minutes. Fresh scan in %d s — stay where you are.", "Android erlaubt 4 WLAN-Suchen alle 2 Minuten. Neue Suche in %d s — bleiben Sie stehen.");
        de("Android limits scans: this spot used %d of %d rounds.", "Android begrenzt die Suchen: dieser Punkt nutzte %d von %d Runden.");
        de("Faster surveys: turn off \"Wi-Fi scan throttling\" in Developer options.", "Schneller messen: \"WLAN-Suchdrosselung\" in den Entwickleroptionen ausschalten.");
        de("Open", "Öffnen");
        de("Testing the download…", "Teste den Download…");
        de("Testing the upload…", "Teste den Upload…");
        de("'%s': %d networks. Walk to the next spot.", "'%s': %d Netzwerke. Gehen Sie zum nächsten Punkt.");
        de("Spot %d", "Punkt %d");
        de("Without permission to see nearby networks, the phone can't measure anything.", "Ohne die Erlaubnis, Netzwerke in der Nähe zu sehen, kann das Telefon nichts messen.");
        de("Android only shows the Wi-Fi networks around you while Location is on. Turn it on, then come back.", "Android zeigt die WLANs in der Umgebung nur, wenn der Standort eingeschaltet ist. Schalten Sie ihn ein und kommen Sie zurück.");
        de("Open settings", "Einstellungen öffnen");
        de("Cancel", "Abbrechen");
        de("Turn Wi-Fi on first.", "Schalten Sie zuerst das WLAN ein.");
        de("Tap where you're standing on the plan first — then the pin lands in the right place in NetSketch.", "Tippen Sie zuerst im Grundriss, wo Sie stehen — dann landet der Punkt in NetSketch an der richtigen Stelle.");
        de("OK", "OK");
        de("Measure anyway", "Trotzdem messen");
        de("The speed test couldn't reach the internet.", "Der Geschwindigkeitstest hat das Internet nicht erreicht.");
        de("No Wi-Fi connection for the speed test, so it was skipped (it never uses mobile data).", "Keine WLAN-Verbindung für den Geschwindigkeitstest, deshalb ausgelassen (er nutzt nie mobile Daten).");
        de("No networks heard here. Try again.", "Hier war kein Netzwerk zu hören. Versuchen Sie es noch einmal.");
        de("Seeing nearby networks needs your precise location: choose \"Precise\" when Android asks.", "Für Netzwerke in der Nähe braucht es Ihren genauen Standort: wählen Sie \"Genau\", wenn Android fragt.");
        de("Open app settings", "App-Einstellungen öffnen");
        de("Cancelled — no point added.", "Abgebrochen — kein Punkt hinzugefügt.");
        // Quick point and ping test
        de("Quick point (signal only)", "Schnellpunkt (nur Signal)");
        de("Connect to your Wi-Fi first: a quick point measures the network you're on.", "Verbinden Sie sich zuerst mit Ihrem WLAN: ein Schnellpunkt misst das Netz, in dem Sie sind.");
        de("Reading your Wi-Fi… %d s. Stand still.", "Lese Ihr WLAN… %d s. Bitte stillstehen.");
        de("'%s': %.0f dBm. Walk to the next spot.", "'%s': %.0f dBm. Gehen Sie zum nächsten Punkt.");
        de("Ping test", "Ping-Test");
        de("Connect to your Wi-Fi first.", "Verbinden Sie sich zuerst mit Ihrem WLAN.");
        de("Couldn't find your router's address.", "Die Adresse Ihres Routers wurde nicht gefunden.");
        de("Pinging your router… %d of %d", "Pinge Ihren Router… %d von %d");
        de("The router didn't answer any of %d tries.", "Der Router hat auf keinen der %d Versuche geantwortet.");
        de("Router ping: %.0f ms average, ±%.0f ms jitter, %d%% lost.", "Router-Ping: %.0f ms im Schnitt, ±%.0f ms Schwankung, %d%% verloren.");
        // Results
        de("Networks here", "Netzwerke hier");
        de("Networks at %s · %d", "Netzwerke bei %s · %d");
        de("Real speed: %.0f down, %.0f up Mbit/s · %.0f ms (±%.0f ms)", "Echte Geschwindigkeit: %.0f runter, %.0f hoch Mbit/s · %.0f ms (±%.0f ms)");
        de("Real speed: %.0f Mbit/s · %.0f ms delay", "Echte Geschwindigkeit: %.0f Mbit/s · %.0f ms Verzögerung");
        de("Testing the Wi-Fi to NetSketch…", "Teste das WLAN zu NetSketch…");
        de("Wi-Fi to NetSketch: %.0f down, %.0f up Mbit/s", "WLAN zu NetSketch: %.0f runter, %.0f hoch Mbit/s");
        // Walk survey
        de("Walk a line (your Wi-Fi only)", "Eine Strecke abgehen (nur Ihr WLAN)");
        de("Stop here", "Hier aufhören");
        de("Walk", "Gang");
        de("Walking · %d readings. Walk slowly and evenly; at the end, tap where you are and press Stop here.", "Unterwegs · %d Messungen. Gehen Sie langsam und gleichmässig; tippen Sie am Ende, wo Sie stehen, und dann auf Hier aufhören.");
        de("A walk needs the floor plan: link NetSketch first.", "Für eine Strecke braucht es den Grundriss: verbinden Sie zuerst NetSketch.");
        de("Tap where the walk starts on the plan first.", "Tippen Sie zuerst im Grundriss, wo die Strecke beginnt.");
        de("Connect to your Wi-Fi first: a walk measures the network you're on.", "Verbinden Sie sich zuerst mit Ihrem WLAN: eine Strecke misst das Netz, in dem Sie sind.");
        de("Tap where you are now on the plan (same storey), then press Stop here.", "Tippen Sie im Grundriss, wo Sie jetzt stehen (gleiches Stockwerk), dann auf Hier aufhören.");
        de("Walk %d: %d points along the line.", "Gang %d: %d Punkte entlang der Strecke.");
        de("(hidden)", "(verborgen)");
        de("  · you", "  · Sie");
        de("  · yours", "  · Ihres");
        de(" · %.0f%% busy", " · %.0f%% belegt");
        de(" · %d devices", " · %d Geräte");
        de("%s ch %d · %d MHz · %s", "%s Kanal %d · %d MHz · %s");
        de("Open (no password)", "Offen (ohne Passwort)");
        de("Channels", "Kanäle");
        de("What this spot shows", "Was dieser Punkt zeigt");
        de("All networks", "Alle Netzwerke");
        de("Show all %d", "Alle %d zeigen");
        // Advice
        de("Weak signal here (%.0f dBm): expect slow speeds and dropouts. An access point closer to this spot, or a cable, helps.", "Schwaches Signal hier (%.0f dBm): langsam und mit Aussetzern. Ein Access Point näher an diesem Punkt oder ein Kabel hilft.");
        de("The phone stays on an access point at %.0f dBm although another one of yours offers %.0f dBm here (a 'sticky client'). Fast roaming (802.11k/v/r) helps devices switch in time.", "Das Telefon bleibt bei einem Access Point mit %.0f dBm, obwohl ein anderer von Ihnen hier %.0f dBm bietet (ein 'klebender Client'). Schnelles Roaming (802.11k/v/r) hilft beim rechtzeitigen Wechsel.");
        de("The phone uses 2.4 GHz although your 5 GHz network is good here (%.0f dBm). Band steering, or one name for both bands, moves devices to the faster band.", "Das Telefon nutzt 2,4 GHz, obwohl Ihr 5-GHz-Netz hier gut ist (%.0f dBm). Band-Steering oder ein Name für beide Bänder bringt Geräte ins schnellere Band.");
        de("Your 2.4 GHz channel %d is shared with %d other networks heard clearly here. Try channel %d.", "Ihr 2,4-GHz-Kanal %d wird mit %d anderen hier deutlich hörbaren Netzen geteilt. Versuchen Sie Kanal %d.");
        de("Channel %d sits between the non-overlapping ones (1, 6, 11 — or 1, 5, 9, 13 in Europe) and collides with neighbours on both sides.", "Kanal %d liegt zwischen den überlappungsfreien (1, 6, 11 — oder 1, 5, 9, 13 in Europa) und stört Nachbarn auf beiden Seiten.");
        de("Your access point reports its channel %.0f%% busy: others take turns on it too.", "Ihr Access Point meldet seinen Kanal zu %.0f%% belegt: andere senden dort auch.");
        de("'%s' is open (no password): anyone nearby can join it.", "'%s' ist offen (ohne Passwort): jeder in der Nähe kann sich verbinden.");
        de("'%s' still uses WEP, which is cracked in minutes. Switch it to WPA3.", "'%s' nutzt noch WEP, das in Minuten geknackt wird. Stellen Sie auf WPA3 um.");
        de("Good spot: strong signal and no crowded channel.", "Guter Punkt: starkes Signal und kein überfüllter Kanal.");
        // Points
        de("Points", "Punkte");
        de("Points · %d", "Punkte · %d");
        de(" · %d not sent yet", " · %d noch nicht gesendet");
        de("None yet. Measure your first spot above.", "Noch keine. Messen Sie oben Ihren ersten Punkt.");
        de("placed on the plan", "im Grundriss platziert");
        de("pin to place on the computer", "Punkt am Computer platzieren");
        de("%d networks · %s", "%d Netzwerke · %s");
        de("Signal only · %s", "Nur Signal · %s");
        de(" · %.0f Mbit/s", " · %.0f Mbit/s");
        de("In NetSketch", "In NetSketch");
        de("On phone", "Auf dem Telefon");
        de("Delete", "Löschen");
        de("Keep", "Behalten");
        de("Delete '%s'? If NetSketch already has it, it stays there.", "'%s' löschen? Wenn NetSketch ihn schon hat, bleibt er dort.");
        de("Rename", "Umbenennen");
        de("Measure again here", "Hier neu messen");
        de("Renaming only changes the name on this phone.", "Umbenennen ändert den Namen nur auf diesem Telefon.");
        de("Spot picked — press Measure here.", "Punkt gewählt — drücken Sie Hier messen.");
        de("Send to NetSketch", "An NetSketch senden");
        de("Save & share file", "Datei speichern & teilen");
        de("Start over", "Neu beginnen");
        de("Delete all %d points on this phone? Points already in NetSketch stay there.", "Alle %d Punkte auf diesem Telefon löschen? Punkte, die schon in NetSketch sind, bleiben dort.");
        de("Your Wi-Fi over %d spots: %.0f to %.0f dBm, %d weak.", "Ihr WLAN über %d Punkte: %.0f bis %.0f dBm, %d schwach.");
        de("Fastest %.0f, slowest %.0f Mbit/s.", "Am schnellsten %.0f, am langsamsten %.0f Mbit/s.");
        de("Tip: measure where you actually use Wi-Fi (desk, sofa, bed), the far corners, and near the walls to your neighbours. Networks you don't own can then be added as neighbours in NetSketch with one click, with the channels and busy time you measured.", "Tipp: Messen Sie dort, wo Sie WLAN wirklich nutzen (Schreibtisch, Sofa, Bett), in den hinteren Ecken und an den Wänden zu den Nachbarn. Fremde Netze können Sie in NetSketch mit einem Klick als Nachbarn hinzufügen, mit den gemessenen Kanälen und der Belegung.");
        // Link
        de("On the computer: NetSketch → overview → Real measurements → Link a phone. Looking for it on this Wi-Fi…", "Am Computer: NetSketch → Übersicht → Echte Messungen → Telefon verbinden. Suche es in diesem WLAN…");
        de("Address shown in NetSketch (if not found)", "In NetSketch angezeigte Adresse (falls nicht gefunden)");
        de("4-digit code", "4-stelliger Code");
        de("Unlink", "Trennen");
        de("Type the address NetSketch shows, e.g. 192.168.1.23:47878.", "Geben Sie die Adresse ein, die NetSketch zeigt, z. B. 192.168.1.23:47878.");
        de("Found NetSketch with '%s'. Enter the code it shows.", "NetSketch mit '%s' gefunden. Geben Sie den angezeigten Code ein.");
        de("Found NetSketch. Enter the code it shows.", "NetSketch gefunden. Geben Sie den angezeigten Code ein.");
        de("Didn't find it automatically (some routers block that). Type the address NetSketch shows.", "Nicht automatisch gefunden (manche Router blockieren das). Geben Sie die Adresse ein, die NetSketch zeigt.");
        de("Enter the 4-digit code NetSketch shows.", "Geben Sie den 4-stelligen Code ein, den NetSketch zeigt.");
        de("unknown error", "unbekannter Fehler");
        de("Linked — tap where you're standing.", "Verbunden — tippen Sie, wo Sie stehen.");
        de("NetSketch says the code is wrong — check the code it shows.", "NetSketch sagt, der Code ist falsch — prüfen Sie den angezeigten Code.");
        de("Couldn't reach NetSketch (%s). Is the computer on the same Wi-Fi and the link switched on?", "NetSketch nicht erreichbar (%s). Ist der Computer im selben WLAN und die Verbindung eingeschaltet?");
        de("The floor plan couldn't be read: %s", "Der Grundriss konnte nicht gelesen werden: %s");
        de("NetSketch already has every point.", "NetSketch hat schon alle Punkte.");
        de("Kept on the phone for now: %s", "Vorerst auf dem Telefon behalten: %s");
        de("Sent to NetSketch.", "An NetSketch gesendet.");
        de("Sent %d points to NetSketch.", "%d Punkte an NetSketch gesendet.");
        // Files
        de("Couldn't keep the points: %s", "Die Punkte konnten nicht gespeichert werden: %s");
        de("The saved points couldn't be read. The damaged file was kept as survey.json.bad.", "Die gespeicherten Punkte konnten nicht gelesen werden. Die beschädigte Datei wurde als survey.json.bad behalten.");
        de("Measure at least one spot first.", "Messen Sie zuerst mindestens einen Punkt.");
        de("Couldn't save to Downloads.", "Konnte nicht in Downloads speichern.");
        de("Couldn't save: %s", "Konnte nicht speichern: %s");
        de("Saved %s in Downloads.", "%s in Downloads gespeichert.");
        de("Wi-Fi survey for NetSketch", "WLAN-Messung für NetSketch");
        de("Send the survey to your computer", "Messung an Ihren Computer senden");
        de("NetSketch Survey %s · GPL-3.0", "NetSketch Survey %s · GPL-3.0");
        // Tabs
        de("Survey", "Messen");
        de("Around you", "In der Nähe");
        de("Watch", "Beobachten");
        de("Check-up", "Check-up");
        de("Devices", "Geräte");
        // Measured point extras
        de("Link: %.0f Mbit/s send, %.0f receive", "Verbindung: %.0f Mbit/s senden, %.0f empfangen");
        de("(at most %.0f)", "(höchstens %.0f)");
        de(" · %.0f to %.0f dBm", " · %.0f bis %.0f dBm");
        de("%.1f m away (RTT)", "%.1f m entfernt (RTT)");
        // Around you
        de("Allow nearby networks to see what's around you.", "Erlauben Sie Netzwerke in der Nähe, um zu sehen, was um Sie herum ist.");
        de("Every network the phone hears, grouped into the boxes that send them. Yours are marked. The list refreshes while it's open.", "Jedes Netz, das das Telefon hört, nach den Geräten gruppiert, die es senden. Ihre sind markiert. Die Liste aktualisiert sich, solange sie offen ist.");
        de("k/v/r: the access point helps devices switch to a nearer one (fast roaming). RTT: it can tell the phone its distance. Android allows 4 scans every 2 minutes, so the list updates about every 30 s.", "k/v/r: der Access Point hilft Geräten, zu einem näheren zu wechseln (schnelles Roaming). RTT: er kann dem Telefon seine Entfernung sagen. Android erlaubt 4 Suchen alle 2 Minuten, deshalb aktualisiert sich die Liste etwa alle 30 s.");
        de("Next scan in %d s", "Nächste Suche in %d s");
        de("Nothing heard yet: the first scan is on its way.", "Noch nichts gehört: die erste Suche läuft.");
        de("%d access points · %d networks heard in the last 5 minutes", "%d Access Points · %d Netze in den letzten 5 Minuten gehört");
        de("%d streams", "%d Streams");
        de("fast roaming %s", "schnelles Roaming %s");
        de("WPS on", "WPS an");
        de("%.0f%% busy", "%.0f%% belegt");
        de("%d devices", "%d Geräte");
        // Watch
        de("Watch my Wi-Fi", "Mein WLAN beobachten");
        de("Watching your Wi-Fi", "Beobachte Ihr WLAN");
        de("Starting…", "Starte…");
        de("Stop", "Stopp");
        de("%d dBm · %d events in the log", "%d dBm · %d Ereignisse im Protokoll");
        de("Wi-Fi is off or gone · %d events in the log", "WLAN ist aus oder weg · %d Ereignisse im Protokoll");
        de("Leave the phone where the trouble happens. Every second it notes the signal, the speed of the link and how fast the router answers, and it scans whenever Android allows. It notices dropouts, delay spikes, switches between access points and routers changing channel. It keeps going with the screen off; plug the phone in for long watches.", "Legen Sie das Telefon dorthin, wo die Probleme auftreten. Jede Sekunde notiert es das Signal, die Verbindungsgeschwindigkeit und wie schnell der Router antwortet, und es sucht, wann immer Android es erlaubt. Es bemerkt Aussetzer, Verzögerungsspitzen, Wechsel zwischen Access Points und Router, die den Kanal wechseln. Es läuft auch bei ausgeschaltetem Bildschirm weiter; für lange Beobachtungen das Telefon ans Ladegerät hängen.");
        de("Start watching", "Beobachtung starten");
        de("Stop watching", "Beobachtung beenden");
        de("Android didn't allow watching in the background, so keep this screen open: it stays on while watching.", "Android hat das Beobachten im Hintergrund nicht erlaubt, lassen Sie also diesen Bildschirm offen: er bleibt beim Beobachten an.");
        de("10 min", "10 Min.");
        de("1 hour", "1 Stunde");
        de("24 hours", "24 Stunden");
        de("Signal and router delay over time", "Signal und Router-Verzögerung im Zeitverlauf");
        de("Line: signal (dBm). Bars: the router's delay. Red: no answer or offline. Marks on top: events.", "Linie: Signal (dBm). Balken: Verzögerung des Routers. Rot: keine Antwort oder offline. Markierungen oben: Ereignisse.");
        de("What happened", "Was passiert ist");
        de("Clear log", "Protokoll löschen");
        de("Delete the watch log on this phone?", "Das Beobachtungsprotokoll auf diesem Telefon löschen?");
        de("Watching for %d h %d min · %d readings · %d events", "Beobachtet seit %d h %d min · %d Messungen · %d Ereignisse");
        de("Nothing recorded yet.", "Noch nichts aufgezeichnet.");
        de("%d readings and %d events from the last 24 hours.", "%d Messungen und %d Ereignisse aus den letzten 24 Stunden.");
        de("No events yet.", "Noch keine Ereignisse.");
        de("'%s' moved from channel %s to %s (%s)", "'%s' wechselte von Kanal %s auf %s (%s)");
        de("Switched access point: %s → %s (%.0f dBm)", "Access Point gewechselt: %s → %s (%.0f dBm)");
        de("Wi-Fi dropped for %.0f s", "WLAN %.0f s lang weg");
        de("Delay spike: %.0f ms", "Verzögerungsspitze: %.0f ms");
        de("The router stopped answering (%.0f%% lost)", "Der Router antwortete nicht mehr (%.0f%% verloren)");
        de("Signal dipped to %.0f dBm", "Signal fiel auf %.0f dBm");
        // Check-up
        de("What the phone can tell about your network: channels and neighbours, your access points' settings, roaming, the watch log, your measured spots and the internet side.", "Was das Telefon über Ihr Netz sagen kann: Kanäle und Nachbarn, die Einstellungen Ihrer Access Points, Roaming, das Beobachtungsprotokoll, Ihre gemessenen Punkte und die Internetseite.");
        de("Check my network", "Mein Netz prüfen");
        de("A phone sees a lot, but not everything: it can't see which way an extender links back, what other devices experience, or the router's own error counters. Findings about that say \"likely\".", "Ein Telefon sieht viel, aber nicht alles: es sieht nicht, wie ein Repeater zurück verbunden ist, was andere Geräte erleben, oder die Fehlerzähler des Routers. Befunde dazu sagen \"wahrscheinlich\".");
        de("Checking the router and the internet side… about 10 seconds.", "Prüfe den Router und die Internetseite… etwa 10 Sekunden.");
        de("Nothing to fix found. Watch for a while, look around, or measure a few spots: then there is more to go on.", "Nichts zu beheben gefunden. Beobachten Sie eine Weile, schauen Sie sich um oder messen Sie ein paar Punkte: dann gibt es mehr Anhaltspunkte.");
        de("Problem", "Problem");
        de("Worth a look", "Ansehen lohnt sich");
        de("Tip", "Tipp");
        de("Press the button to also check the router and the internet side.", "Drücken Sie den Knopf, um auch den Router und die Internetseite zu prüfen.");
        // Devices
        de("Devices on your network", "Geräte in Ihrem Netz");
        de("Finds what's connected: devices that announce themselves (TVs, printers, speakers, the router itself) and any address that answers a knock.", "Findet, was verbunden ist: Geräte, die sich selbst melden (Fernseher, Drucker, Lautsprecher, der Router selbst) und jede Adresse, die auf ein Anklopfen antwortet.");
        de("Find devices", "Geräte suchen");
        de("Android hides other devices' hardware addresses from apps, so a device that doesn't announce itself shows only its address. Phones often ignore knocks and may be missing.", "Android verbirgt die Hardware-Adressen anderer Geräte vor Apps, deshalb zeigt ein Gerät, das sich nicht meldet, nur seine Adresse. Telefone ignorieren Anklopfen oft und können fehlen.");
        de("Nothing searched yet.", "Noch nicht gesucht.");
        de("%d devices found.", "%d Geräte gefunden.");
        de("Unknown device", "Unbekanntes Gerät");
        de("This phone", "Dieses Telefon");
        de("Asking UPnP devices…", "Frage UPnP-Geräte…");
        de("Knocking on every address…", "Klopfe bei jeder Adresse an…");
        de("Listening for printers, TVs and speakers…", "Lausche nach Druckern, Fernsehern und Lautsprechern…");
        de("Router", "Router");
        de("Access point", "Access Point");
        de("Printer", "Drucker");
        de("TV or speaker (Chromecast)", "Fernseher oder Lautsprecher (Chromecast)");
        de("TV or speaker", "Fernseher oder Lautsprecher");
        de("Apple TV or AirPlay device", "Apple TV oder AirPlay-Gerät");
        de("AirPlay speaker", "AirPlay-Lautsprecher");
        de("Smart-home device", "Smart-Home-Gerät");
        de("Computer or NAS", "Computer oder NAS");
        de("Speaker", "Lautsprecher");
        de("Device with a web page", "Gerät mit Webseite");
        de("Media server", "Medienserver");
        de("iPhone or iPad", "iPhone oder iPad");
        de("Camera", "Kamera");
        // Assistant
        de("Your %s network '%s' on channel %d shares the air with %d neighbouring networks (loudest: '%s', %.0f dBm).", "Ihr %s-Netz '%s' auf Kanal %d teilt sich die Luft mit %d Nachbarnetzen (am lautesten: '%s', %.0f dBm).");
        de("Channel %d is the quietest here.", "Kanal %d ist hier am ruhigsten.");
        de("Your access point %s changed its %s channel %d times (%s). Each change cuts devices off for a moment. Automatic channel selection does this: set a fixed channel.", "Ihr Access Point %s hat seinen %s-Kanal %d-mal gewechselt (%s). Jeder Wechsel trennt Geräte kurz. Das macht die automatische Kanalwahl: stellen Sie einen festen Kanal ein.");
        de("%d neighbouring access points change channel by themselves, too.", "%d benachbarte Access Points wechseln ebenfalls selbst den Kanal.");
        de("a fixed channel", "einen festen Kanal");
        de("channel %d", "Kanal %d");
        de("Your router and a neighbour ('%s') keep changing %s channel around each other (yours %s → %s at %s, theirs %s → %s at %s). Both pick channels automatically and chase the same quiet spot. Set yours to %s and this stops.", "Ihr Router und ein Nachbar ('%s') wechseln den %s-Kanal immer wieder umeinander herum (Ihrer %s → %s um %s, seiner %s → %s um %s). Beide wählen den Kanal automatisch und jagen derselben ruhigen Stelle nach. Stellen Sie Ihren auf %s, dann hört das auf.");
        de("'%s' moved onto your %s channel %s at %s.", "'%s' ist auf Ihren %s-Kanal %s gewechselt (um %s).");
        de("Your 2.4 GHz network '%s' uses 40 MHz. On 2.4 GHz that covers most of the band and collides with every neighbour: set it to 20 MHz (the speed comes from 5 GHz).", "Ihr 2,4-GHz-Netz '%s' nutzt 40 MHz. Auf 2,4 GHz deckt das fast das ganze Band ab und stört jeden Nachbarn: stellen Sie 20 MHz ein (die Geschwindigkeit kommt von 5 GHz).");
        de("'%s' uses 160 MHz on 5 GHz, where %d neighbouring networks overlap it. 80 MHz is often faster here: less air to share, fewer collisions.", "'%s' nutzt 160 MHz auf 5 GHz, wo sich %d Nachbarnetze damit überschneiden. 80 MHz ist hier oft schneller: weniger zu teilen, weniger Kollisionen.");
        de("Your 5 GHz network '%s' is on channel %d, a radar channel (DFS): when the router hears radar it must leave for a while and devices drop briefly. If you see short dropouts, pick one of 36–48.", "Ihr 5-GHz-Netz '%s' ist auf Kanal %d, einem Radarkanal (DFS): hört der Router Radar, muss er ihn eine Weile verlassen und Geräte fallen kurz heraus. Bei kurzen Aussetzern wählen Sie einen von 36–48.");
        de("WPS is switched on for '%s'. Its PIN method can be cracked: turn WPS off in the router unless you are pairing something right now.", "WPS ist für '%s' eingeschaltet. Die PIN-Methode lässt sich knacken: schalten Sie WPS im Router aus, außer Sie koppeln gerade etwas.");
        de("'%s' still allows the old TKIP encryption: it's weak, and devices using it are held to 54 Mbit/s. Choose WPA2 (AES) or WPA3.", "'%s' erlaubt noch die alte TKIP-Verschlüsselung: sie ist schwach, und Geräte damit bleiben bei 54 Mbit/s. Wählen Sie WPA2 (AES) oder WPA3.");
        de("'%s' uses only WPA2 although it runs on %s gear, which can do WPA3. The 'WPA2/WPA3' mode keeps older devices working and protects the rest better.", "'%s' nutzt nur WPA2, obwohl es auf %s-Hardware läuft, die WPA3 kann. Der Modus 'WPA2/WPA3' hält ältere Geräte am Laufen und schützt den Rest besser.");
        de("Your access point %s reports its channel %.0f%% busy: devices wait their turn, so everything feels slower.", "Ihr Access Point %s meldet seinen Kanal zu %.0f%% belegt: Geräte warten, bis sie dran sind, deshalb fühlt sich alles langsamer an.");
        de("Your access points announce different countries (%s). Set the same country everywhere, or some devices won't use some channels.", "Ihre Access Points melden verschiedene Länder (%s). Stellen Sie überall dasselbe Land ein, sonst nutzen manche Geräte manche Kanäle nicht.");
        de("'%s' (2.4 GHz) and '%s' (5 GHz) are separate names on one box. With one name for both, devices pick the faster band themselves and switch when it fades.", "'%s' (2,4 GHz) und '%s' (5 GHz) sind getrennte Namen auf einem Gerät. Mit einem Namen für beide wählen Geräte das schnellere Band selbst und wechseln, wenn es nachlässt.");
        de("'%s' has its own name, so devices don't roam between it and '%s': they hold on to one until it fails. Give the extender the same name and password (or use mesh).", "'%s' hat einen eigenen Namen, deshalb wechseln Geräte nicht zwischen ihm und '%s': sie halten an einem fest, bis es versagt. Geben Sie dem Repeater denselben Namen und dasselbe Passwort (oder nutzen Sie Mesh).");
        de("'%s' comes from %d access points, but none of them announces fast roaming (802.11k/v/r). Switching it on in the router (often called 'fast roaming' or 'steering') helps devices move to the nearer one.", "'%s' kommt von %d Access Points, aber keiner meldet schnelles Roaming (802.11k/v/r). Es im Router einzuschalten (oft 'Fast Roaming' oder 'Steering' genannt) hilft Geräten, zum näheren zu wechseln.");
        de("For the last few minutes the phone stayed on %s at %.0f dBm although %s offered %.0f dBm. Devices that stick like this are slow; fast roaming (802.11k/v/r), or a little less transmit power on the far one, helps.", "In den letzten Minuten blieb das Telefon bei %s mit %.0f dBm, obwohl %s %.0f dBm bot. Geräte, die so kleben, sind langsam; schnelles Roaming (802.11k/v/r) oder etwas weniger Sendeleistung beim fernen hilft.");
        de("At %d of your spots the phone stayed on a far access point although a nearer one of yours was at least 10 dB stronger.", "An %d Ihrer Punkte blieb das Telefon bei einem fernen Access Point, obwohl ein näherer von Ihnen mindestens 10 dB stärker war.");
        de("The phone switched back and forth between %s and %s %d times. Both reach this spot about equally well, so devices keep hopping; lower the transmit power of one, or move it a little.", "Das Telefon wechselte zwischen %s und %s hin und her, %d-mal. Beide erreichen diesen Punkt etwa gleich gut, deshalb springen Geräte ständig; senken Sie die Sendeleistung von einem oder verschieben Sie ihn etwas.");
        de("Wi-Fi dropped %d times while watching (the longest for %.0f s). Where the signal is good, that points to the router restarting or overloaded.", "Das WLAN fiel beim Beobachten %d-mal aus (am längsten %.0f s). Wo das Signal gut ist, deutet das auf einen Router, der neu startet oder überlastet ist.");
        de("Wi-Fi dropped once while watching, for %.0f s.", "Das WLAN fiel beim Beobachten einmal aus, für %.0f s.");
        de("The delay jumps about every %.0f s (%d times): something runs on a timer, often power saving or the phone or router scanning in the background.", "Die Verzögerung springt etwa alle %.0f s (%d-mal): etwas läuft nach Zeitplan, oft Energiesparen oder das Telefon oder der Router, die im Hintergrund suchen.");
        de("The router's delay jumped above 100 ms %d times (worst %.0f ms). Video calls and games stutter then.", "Die Verzögerung des Routers sprang %d-mal über 100 ms (schlimmstenfalls %.0f ms). Videoanrufe und Spiele ruckeln dann.");
        de("The router stopped answering in bursts %d times: the connection hiccups even when it stays up.", "Der Router hörte %d-mal zeitweise auf zu antworten: die Verbindung stockt, auch wenn sie bestehen bleibt.");
        de("The signal dropped sharply %d times (lowest %.0f dBm). People, doors or a running microwave in the way can do this.", "Das Signal fiel %d-mal stark ab (am tiefsten %.0f dBm). Menschen, Türen oder eine laufende Mikrowelle im Weg können das bewirken.");
        de("A phone can't see which way an extender really connects: this is an estimate from where its signal peaked.", "Ein Telefon sieht nicht, wie ein Repeater wirklich verbunden ist: das ist eine Schätzung danach, wo sein Signal am stärksten war.");
        de("%s: where its signal is strongest ('%s'), your main router (probably %s) reached only %s. Its own link back is likely weak, which slows everything connected to it. Move it a little closer to the router, or connect it with a cable.", "%s: wo sein Signal am stärksten ist ('%s'), erreichte Ihr Hauptrouter (wahrscheinlich %s) nur %s. Seine eigene Verbindung zurück ist wahrscheinlich schwach, was alles bremst, was mit ihm verbunden ist. Stellen Sie ihn etwas näher an den Router oder verbinden Sie ihn mit einem Kabel.");
        de("nothing", "nichts");
        de("%.0f dBm", "%.0f dBm");
        de("At '%s', near %s, %s was %.0f dB louder than your main router. If %s links straight to the router, it uses the weaker path; if it links through %s, data crosses the air twice. Check its uplink in the router's or mesh app's overview.", "Bei '%s', nahe %s, war %s %.0f dB lauter als Ihr Hauptrouter. Verbindet sich %s direkt mit dem Router, nutzt er den schwächeren Weg; verbindet er sich über %s, gehen die Daten zweimal durch die Luft. Prüfen Sie seine Verbindung in der Übersicht des Routers oder der Mesh-App.");
        de("Connected to %s with a strong signal, the phone got only %.0f Mbit/s, against %.0f Mbit/s at your other access points. Its link back to the router is probably the bottleneck.", "Mit starkem Signal an %s bekam das Telefon nur %.0f Mbit/s, gegenüber %.0f Mbit/s an Ihren anderen Access Points. Seine Verbindung zurück zum Router ist wahrscheinlich der Engpass.");
        de("Your provider shares one public address among many customers (carrier-grade NAT; your router got %s). Port forwarding, and some games or cameras, won't work from outside: ask for a public IPv4 address, or use IPv6.", "Ihr Anbieter teilt eine öffentliche Adresse unter vielen Kunden (Carrier-Grade-NAT; Ihr Router bekam %s). Portweiterleitungen und manche Spiele oder Kameras funktionieren von außen nicht: fragen Sie nach einer öffentlichen IPv4-Adresse oder nutzen Sie IPv6.");
        de("Double NAT: your router's internet side has the private address %s, so it sits behind another router (often the provider's modem). Put that one in bridge mode, or make yours the only router.", "Doppeltes NAT: die Internetseite Ihres Routers hat die private Adresse %s, er sitzt also hinter einem weiteren Router (oft dem Modem des Anbieters). Stellen Sie jenen auf Bridge-Modus oder machen Sie Ihren zum einzigen Router.");
        de("Your router's internet address (%s) differs from the one the internet sees (%s): something between you and the internet translates addresses again (double NAT or carrier-grade NAT).", "Die Internetadresse Ihres Routers (%s) unterscheidet sich von der, die das Internet sieht (%s): etwas zwischen Ihnen und dem Internet übersetzt die Adressen noch einmal (doppeltes NAT oder Carrier-Grade-NAT).");
        de("Your router introduces itself as %s.", "Ihr Router stellt sich als %s vor.");
        de("No IPv6 on this Wi-Fi. Most providers offer it, and it avoids their address sharing: switch it on in the router if you can.", "Kein IPv6 in diesem WLAN. Die meisten Anbieter bieten es an, und es umgeht deren Adressteilung: schalten Sie es im Router ein, wenn möglich.");
        de("Looking up a name takes %.0f ms here (good is under 50 ms). Every new website waits for it: try another DNS server in the router.", "Eine Namensauflösung dauert hier %.0f ms (gut ist unter 50 ms). Jede neue Webseite wartet darauf: versuchen Sie im Router einen anderen DNS-Server.");
        de("Names are looked up by %s.", "Namen löst %s auf.");
        de("Names are looked up by %s (encrypted, Private DNS).", "Namen löst %s auf (verschlüsselt, privates DNS).");
        de("Couldn't start watching: %s", "Beobachten konnte nicht starten: %s");
    }
}
