package org.netsketch.survey;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.view.GestureDetector;
import android.view.MotionEvent;
import android.view.ScaleGestureDetector;
import android.view.View;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/**
 * The floor plan from NetSketch: walls, room names, devices and measuring points of one storey.
 * Tap where you stand; that spot (in metres, like the plan) goes with the next measurement.
 * Pinch to zoom, drag with two fingers (or one, once zoomed) to move, double-tap to fit again.
 */
final class PlanView extends View {
    static final class Wall {
        int floor;
        float ax, ay, bx, by, thick;
        String kind = "";
    }

    static final class Label {
        int floor;
        float x, y;
        String text;
    }

    static final class Dev {
        int floor;
        float x, y;
        String name;
        boolean radio;
    }

    /** A point measured on this phone (coloured by your Wi-Fi there), or one NetSketch already has. */
    static final class Pin {
        int floor;
        float x, y;
        String name;
        /** Your Wi-Fi's strength there, or NaN if unknown; points from NetSketch are drawn hollow. */
        double rssi = Double.NaN;
        boolean fromComputer;
        Object tag;
    }

    interface OnSpot {
        void picked(float x, float y);
    }

    interface OnPin {
        void tapped(Pin pin);
    }

    final List<Wall> walls = new ArrayList<>();
    final List<Label> labels = new ArrayList<>();
    final List<Dev> devices = new ArrayList<>();
    final List<Pin> pins = new ArrayList<>();
    int floor;
    /** The spot picked for the next measurement (metres), or null. */
    Float selX, selY;
    OnSpot onSpot;
    OnPin onPin;

    private final Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);
    private float scale = 1, offX, offY;
    /** The person's zoom and pan on top of the fitted view. */
    private float zoom = 1, panX, panY;
    private boolean moved;
    private float lastX, lastY;
    private final ScaleGestureDetector pinch;
    private final GestureDetector taps;

    PlanView(Context c) {
        super(c);
        setBackground(Ui.round(c, Ui.BG, 14, Ui.LINE));
        pinch = new ScaleGestureDetector(c, new ScaleGestureDetector.SimpleOnScaleGestureListener() {
            @Override
            public boolean onScale(ScaleGestureDetector d) {
                float z = Math.max(1, Math.min(8, zoom * d.getScaleFactor()));
                float f = z / zoom;
                // Keep the spot between the fingers where it is.
                panX = d.getFocusX() - (d.getFocusX() - panX) * f;
                panY = d.getFocusY() - (d.getFocusY() - panY) * f;
                zoom = z;
                moved = true;
                invalidate();
                return true;
            }
        });
        taps = new GestureDetector(c, new GestureDetector.SimpleOnGestureListener() {
            @Override
            public boolean onSingleTapConfirmed(MotionEvent e) {
                tap(e.getX(), e.getY());
                return true;
            }

            @Override
            public boolean onDoubleTap(MotionEvent e) {
                zoom = 1;
                panX = 0;
                panY = 0;
                invalidate();
                return true;
            }
        });
    }

    /** Forgets the plan, but keeps the spot the person picked (the plan is fetched again now and then). */
    void clear() {
        walls.clear();
        labels.clear();
        devices.clear();
        pins.clear();
    }

    void unpick() {
        selX = null;
        selY = null;
        describe();
        invalidate();
    }

    void pick(float x, float y) {
        selX = x;
        selY = y;
        describe();
        invalidate();
    }

    /** Fits this storey into the view, with room around the edges; then applies the person's zoom. */
    private void fit() {
        float x0 = Float.MAX_VALUE, y0 = Float.MAX_VALUE, x1 = -Float.MAX_VALUE, y1 = -Float.MAX_VALUE;
        for (Wall w : walls) {
            if (w.floor != floor) continue;
            x0 = Math.min(x0, Math.min(w.ax, w.bx));
            x1 = Math.max(x1, Math.max(w.ax, w.bx));
            y0 = Math.min(y0, Math.min(w.ay, w.by));
            y1 = Math.max(y1, Math.max(w.ay, w.by));
        }
        for (Dev d : devices) {
            if (d.floor != floor) continue;
            x0 = Math.min(x0, d.x);
            x1 = Math.max(x1, d.x);
            y0 = Math.min(y0, d.y);
            y1 = Math.max(y1, d.y);
        }
        if (x0 > x1) {
            x0 = 0;
            y0 = 0;
            x1 = 10;
            y1 = 8;
        }
        float pad = Ui.dp(getContext(), 22);
        float w = Math.max(1, x1 - x0), h = Math.max(1, y1 - y0);
        float base = Math.min((getWidth() - 2 * pad) / w, (getHeight() - 2 * pad) / h);
        float bx = (getWidth() - w * base) / 2 - x0 * base;
        float by = (getHeight() - h * base) / 2 - y0 * base;
        scale = base * zoom;
        offX = bx * zoom + panX;
        offY = by * zoom + panY;
    }

    private float sx(float x) {
        return offX + x * scale;
    }

    private float sy(float y) {
        return offY + y * scale;
    }

    @Override
    protected void onDraw(Canvas cv) {
        fit();
        Context c = getContext();
        p.setStrokeCap(Paint.Cap.ROUND);
        for (Wall w : walls) {
            if (w.floor != floor) continue;
            p.setStyle(Paint.Style.STROKE);
            p.setColor(wallColor(w.kind));
            p.setStrokeWidth(Math.max(Ui.dp(c, 2), wallThick(w.kind) * scale));
            cv.drawLine(sx(w.ax), sy(w.ay), sx(w.bx), sy(w.by), p);
        }
        p.setStyle(Paint.Style.FILL);
        p.setTextAlign(Paint.Align.CENTER);
        p.setTextSize(Ui.dp(c, 12));
        p.setColor(Ui.DIM);
        for (Label l : labels) {
            if (l.floor == floor) cv.drawText(l.text, sx(l.x), sy(l.y), p);
        }
        float r = Ui.dp(c, 5);
        p.setTextSize(Ui.dp(c, 9));
        for (Dev d : devices) {
            if (d.floor != floor) continue;
            p.setColor(d.radio ? Ui.ACCENT : Ui.BLUE);
            cv.drawCircle(sx(d.x), sy(d.y), r, p);
            p.setColor(Ui.DIM);
            cv.drawText(d.name, sx(d.x), sy(d.y) + r + Ui.dp(c, 10), p);
        }
        for (Pin q : pins) {
            if (q.floor != floor) continue;
            if (q.fromComputer) {
                p.setStyle(Paint.Style.STROKE);
                p.setStrokeWidth(Ui.dp(c, 1.5f));
                p.setColor(Ui.DIM);
                cv.drawCircle(sx(q.x), sy(q.y), r * 0.8f, p);
                p.setStyle(Paint.Style.FILL);
            } else {
                p.setColor(Double.isNaN(q.rssi) ? Ui.AMBER : Ui.quality(q.rssi));
                cv.drawCircle(sx(q.x), sy(q.y), r * 1.1f, p);
                if (!Double.isNaN(q.rssi) && zoom >= 1.5f) {
                    p.setColor(Ui.TEXT);
                    cv.drawText(String.format(Locale.ROOT, "%.0f", q.rssi), sx(q.x), sy(q.y) - r * 1.6f, p);
                }
            }
        }
        if (selX != null && selY != null) {
            float x = sx(selX), y = sy(selY);
            p.setColor(Color.argb(60, Color.red(Ui.ACCENT), Color.green(Ui.ACCENT), Color.blue(Ui.ACCENT)));
            cv.drawCircle(x, y, r * 4, p);
            p.setStyle(Paint.Style.STROKE);
            p.setStrokeWidth(Ui.dp(c, 2.5f));
            p.setColor(Ui.ACCENT);
            cv.drawCircle(x, y, r * 2.2f, p);
            p.setStyle(Paint.Style.FILL);
            cv.drawCircle(x, y, r * 0.9f, p);
        }
        if (!walls.isEmpty()) {
            p.setColor(Ui.DIM);
            p.setTextSize(Ui.dp(c, 11));
            String hint = selX == null ? T.t("Tap where you're standing") : T.t("Pinch to zoom · double-tap to fit");
            cv.drawText(hint, getWidth() / 2f, getHeight() - Ui.dp(c, 8), p);
        }
    }

    @Override
    public boolean onTouchEvent(MotionEvent e) {
        pinch.onTouchEvent(e);
        taps.onTouchEvent(e);
        switch (e.getActionMasked()) {
            case MotionEvent.ACTION_DOWN:
                moved = false;
                lastX = e.getX();
                lastY = e.getY();
                getParent().requestDisallowInterceptTouchEvent(true);
                break;
            case MotionEvent.ACTION_POINTER_DOWN:
            case MotionEvent.ACTION_POINTER_UP:
                lastX = midX(e);
                lastY = midY(e);
                break;
            case MotionEvent.ACTION_MOVE: {
                float x = midX(e), y = midY(e);
                // One finger only moves the plan once it's zoomed in; otherwise the page scrolls.
                if (e.getPointerCount() > 1 || zoom > 1.01f) {
                    panX += x - lastX;
                    panY += y - lastY;
                    if (Math.hypot(x - lastX, y - lastY) > 1) moved = true;
                    invalidate();
                } else if (Math.hypot(e.getX() - lastX, e.getY() - lastY) > Ui.dp(getContext(), 12)) {
                    getParent().requestDisallowInterceptTouchEvent(false);
                }
                lastX = x;
                lastY = y;
                break;
            }
            case MotionEvent.ACTION_UP:
            case MotionEvent.ACTION_CANCEL:
                getParent().requestDisallowInterceptTouchEvent(false);
                break;
            default:
                break;
        }
        return true;
    }

    private static float midX(MotionEvent e) {
        float s = 0;
        int n = e.getPointerCount();
        for (int i = 0; i < n; i++) if (!(e.getActionMasked() == MotionEvent.ACTION_POINTER_UP && i == e.getActionIndex())) s += e.getX(i);
        return s / Math.max(1, n - (e.getActionMasked() == MotionEvent.ACTION_POINTER_UP ? 1 : 0));
    }

    private static float midY(MotionEvent e) {
        float s = 0;
        int n = e.getPointerCount();
        for (int i = 0; i < n; i++) if (!(e.getActionMasked() == MotionEvent.ACTION_POINTER_UP && i == e.getActionIndex())) s += e.getY(i);
        return s / Math.max(1, n - (e.getActionMasked() == MotionEvent.ACTION_POINTER_UP ? 1 : 0));
    }

    /** A tap picks the spot; a tap on one of this phone's points also shows what was measured there. */
    private void tap(float x, float y) {
        if (moved || scale <= 0) return;
        Pin hit = null;
        double hd = Ui.dp(getContext(), 16);
        for (Pin q : pins) {
            if (q.floor != floor || q.fromComputer) continue;
            double d = Math.hypot(sx(q.x) - x, sy(q.y) - y);
            if (d < hd) {
                hd = d;
                hit = q;
            }
        }
        if (hit != null && onPin != null) {
            onPin.tapped(hit);
            return;
        }
        pick((x - offX) / scale, (y - offY) / scale);
        if (onSpot != null) onSpot.picked(selX, selY);
        performClick();
    }

    @Override
    public boolean performClick() {
        return super.performClick();
    }

    private void describe() {
        String where = selX == null ? T.t("Tap where you're standing") : T.f("Spot picked: %.1f m, %.1f m — measure when you're standing there.", selX, selY);
        setContentDescription(T.f("Floor plan. %s", where));
    }

    /** The room name nearest to a spot on this storey (within 6 m), for naming the point. */
    String roomAt(float x, float y) {
        String best = null;
        double bd = 6;
        for (Label l : labels) {
            if (l.floor != floor) continue;
            double d = Math.hypot(l.x - x, l.y - y);
            if (d < bd) {
                bd = d;
                best = l.text;
            }
        }
        return best;
    }

    static int wallColor(String kind) {
        boolean d = Ui.dark;
        switch (kind) {
            case "Concrete": return d ? Color.rgb(120, 130, 145) : Color.rgb(90, 100, 115);
            case "Brick": return Color.rgb(190, 90, 70);
            case "Wood": return Color.rgb(180, 130, 80);
            case "Glass": case "LowE": return d ? Color.rgb(125, 211, 252) : Color.rgb(14, 165, 233);
            case "Metal": return d ? Color.rgb(200, 200, 210) : Color.rgb(70, 75, 90);
            default: return d ? Color.rgb(160, 170, 185) : Color.rgb(120, 130, 148);
        }
    }

    static float wallThick(String kind) {
        switch (kind) {
            case "Concrete": return 0.25f;
            case "Brick": return 0.2f;
            case "Drywall": return 0.1f;
            case "Wood": return 0.08f;
            default: return 0.05f;
        }
    }
}
