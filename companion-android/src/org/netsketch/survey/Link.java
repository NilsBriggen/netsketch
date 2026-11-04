package org.netsketch.survey;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.DatagramPacket;
import java.net.DatagramSocket;
import java.net.HttpURLConnection;
import java.net.InetAddress;
import java.net.InterfaceAddress;
import java.net.NetworkInterface;
import java.net.SocketTimeoutException;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * The live link to NetSketch on a computer in the same Wi-Fi. Plain Java (no Android classes), so it
 * can be checked on a computer. NetSketch answers a broadcast "NETSKETCH?" on UDP 47879 with
 * "NETSKETCH <port> <plan name>"; then GET /plan and POST /points with the 4-digit code.
 */
public final class Link {
    public static final int DISCOVERY_PORT = 47879;
    public static final int DEFAULT_PORT = 47878;

    public final String host;
    public final int port;
    public final String code;
    public final String title;

    public Link(String host, int port, String code, String title) {
        this.host = host;
        this.port = port;
        this.code = code;
        this.title = title == null ? "" : title;
    }

    public Link withCode(String c) {
        return new Link(host, port, c, title);
    }

    /** Looks for NetSketch on the local network; null if nothing answers in time. */
    public static Link discover(int timeoutMs) throws IOException {
        // Ask everywhere at once: the general broadcast, and each network's own broadcast address
        // (some routers drop the general one).
        List<InetAddress> targets = new ArrayList<>();
        targets.add(InetAddress.getByName("255.255.255.255"));
        for (NetworkInterface ni : Collections.list(NetworkInterface.getNetworkInterfaces())) {
            if (!ni.isUp() || ni.isLoopback()) continue;
            for (InterfaceAddress a : ni.getInterfaceAddresses()) {
                if (a.getBroadcast() != null) targets.add(a.getBroadcast());
            }
        }
        return ask(targets, timeoutMs);
    }

    /** Asks one address directly (also how the checks on the computer reach NetSketch). */
    public static Link discoverAt(InetAddress target, int timeoutMs) throws IOException {
        return ask(Collections.singletonList(target), timeoutMs);
    }

    private static Link ask(List<InetAddress> targets, int timeoutMs) throws IOException {
        try (DatagramSocket s = new DatagramSocket()) {
            s.setBroadcast(true);
            s.setSoTimeout(timeoutMs);
            byte[] ask = "NETSKETCH?".getBytes(StandardCharsets.US_ASCII);
            for (InetAddress t : targets) {
                try {
                    s.send(new DatagramPacket(ask, ask.length, t, DISCOVERY_PORT));
                } catch (IOException ignored) {
                    // that network doesn't allow it; the others may
                }
            }
            byte[] buf = new byte[256];
            DatagramPacket in = new DatagramPacket(buf, buf.length);
            try {
                s.receive(in);
            } catch (SocketTimeoutException e) {
                return null;
            }
            return parseAnswer(new String(in.getData(), 0, in.getLength(), StandardCharsets.UTF_8), in.getAddress().getHostAddress());
        }
    }

    /** "NETSKETCH 47878 Small flat" from 192.168.1.23 → a link without a code yet. */
    public static Link parseAnswer(String answer, String from) {
        if (answer == null || !answer.startsWith("NETSKETCH ")) return null;
        String[] parts = answer.split(" ", 3);
        try {
            return new Link(from, Integer.parseInt(parts[1].trim()), "", parts.length > 2 ? parts[2] : "");
        } catch (NumberFormatException | ArrayIndexOutOfBoundsException e) {
            return null;
        }
    }

    /** "192.168.1.23" or "192.168.1.23:47878", typed by hand. */
    public static Link parseAddress(String typed) {
        String t = typed == null ? "" : typed.trim();
        if (t.isEmpty()) return null;
        int colon = t.lastIndexOf(':');
        try {
            if (colon > 0) return new Link(t.substring(0, colon), Integer.parseInt(t.substring(colon + 1)), "", "");
        } catch (NumberFormatException e) {
            return null;
        }
        return new Link(t, DEFAULT_PORT, "", "");
    }

    public String plan() throws IOException {
        return http("GET", "/plan", null);
    }

    public void send(String surveyJson) throws IOException {
        http("POST", "/points", surveyJson);
    }

    /** Thrown when NetSketch says the code is wrong. */
    public static final class WrongCode extends IOException {
        WrongCode() {
            super("wrong code");
        }
    }

    private String http(String method, String path, String body) throws IOException {
        URL url = new URL("http://" + host + ":" + port + path + "?code=" + code);
        HttpURLConnection c = (HttpURLConnection) url.openConnection();
        c.setConnectTimeout(4000);
        c.setReadTimeout(8000);
        c.setRequestMethod(method);
        try {
            if (body != null) {
                byte[] b = body.getBytes(StandardCharsets.UTF_8);
                c.setDoOutput(true);
                c.setFixedLengthStreamingMode(b.length);
                c.setRequestProperty("Content-Type", "application/json");
                try (OutputStream out = c.getOutputStream()) {
                    out.write(b);
                }
            }
            int status = c.getResponseCode();
            if (status == 403) throw new WrongCode();
            if (status != 200) throw new IOException("NetSketch answered " + status);
            try (InputStream in = c.getInputStream()) {
                ByteArrayOutputStream all = new ByteArrayOutputStream();
                byte[] buf = new byte[16 * 1024];
                int n;
                while ((n = in.read(buf)) > 0) all.write(buf, 0, n);
                return all.toString("UTF-8");
            }
        } finally {
            c.disconnect();
        }
    }
}
