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
        return String.format(Locale.ROOT, t(en), args);
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
        de("Waiting for Android… %d s", "Warte auf Android… %d s");
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
        // Results
        de("Networks here", "Netzwerke hier");
        de("Networks at %s · %d", "Netzwerke bei %s · %d");
        de("Real speed: %.0f down, %.0f up Mbit/s · %.0f ms (±%.0f ms)", "Echte Geschwindigkeit: %.0f runter, %.0f hoch Mbit/s · %.0f ms (±%.0f ms)");
        de("Real speed: %.0f Mbit/s · %.0f ms delay", "Echte Geschwindigkeit: %.0f Mbit/s · %.0f ms Verzögerung");
        de("(hidden)", "(verborgen)");
        de("  · you", "  · Sie");
        de("  · yours", "  · Ihres");
        de(" · %.0f%% busy", " · %.0f%% belegt");
        de(" · %d devices", " · %d Geräte");
        de("%s ch %d · %d MHz · %s", "%s Kanal %d · %d MHz · %s");
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
        de("Measure at least one spot first.", "Messen Sie zuerst mindestens einen Punkt.");
        de("Couldn't save to Downloads.", "Konnte nicht in Downloads speichern.");
        de("Couldn't save: %s", "Konnte nicht speichern: %s");
        de("Saved %s in Downloads.", "%s in Downloads gespeichert.");
        de("Wi-Fi survey for NetSketch", "WLAN-Messung für NetSketch");
        de("Send the survey to your computer", "Messung an Ihren Computer senden");
        de("NetSketch Survey %s · GPL-3.0", "NetSketch Survey %s · GPL-3.0");
    }
}
