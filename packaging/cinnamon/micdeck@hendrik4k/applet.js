// MicDeck in the Cinnamon panel: an icon for the receiver's state and a
// menu that opens when the mouse rests on it (or on click) with status,
// live level, mute and the dock switches.
//
// Talks to the MicDeck app over the session bus (io.github.hendrik4k.MicDeck,
// see app/src-tauri/src/panel_bus.rs). While this applet owns
// io.github.hendrik4k.MicDeck.Applet, MicDeck hides its own tray icon.

const Applet = imports.ui.applet;
const PopupMenu = imports.ui.popupMenu;
const Mainloop = imports.mainloop;
const St = imports.gi.St;
const Gio = imports.gi.Gio;
const GLib = imports.gi.GLib;
const Clutter = imports.gi.Clutter;

const BUS = "io.github.hendrik4k.MicDeck";
const PATH = "/io/github/hendrik4k/MicDeck";
const APPLET_NAME = "io.github.hendrik4k.MicDeck.Applet";

const IFACE = `<node><interface name="io.github.hendrik4k.MicDeck">
  <method name="State"><arg type="s" direction="out"/></method>
  <method name="SetMuted"><arg type="b" direction="in"/></method>
  <method name="ShowWindow"/>
  <method name="RestartReceiver"/>
  <method name="SetAppSetting"><arg type="s" direction="in"/><arg type="b" direction="in"/></method>
  <signal name="StateChanged"><arg type="s"/></signal>
  <signal name="Level"><arg type="d"/></signal>
</interface></node>`;
const MicDeckProxy = Gio.DBusProxy.makeProxyWrapper(IFACE);

// Hover: open after this long resting on the icon, close this long after
// the pointer left both icon and menu.
const OPEN_DELAY_MS = 250;
const CLOSE_DELAY_MS = 450;
const SENT_SHOWN_MS = 1500;
const PULSE_MS = 700;

const SEGMENTS = 20;
const FLOOR_DB = -60;

const COLORS = {
    ok: "#3ddc68", warn: "#ffc233", error: "#ff4b4b", pending: "#19d0c0", off: "#8d7f96",
    accent: "#ff8a1f",
};

const TEXT = {
    de: {
        notRunning: "MicDeck läuft nicht", start: "MicDeck starten", open: "MicDeck öffnen",
        mute: "Stummschalten", dockDictation: "Diktat über Ladecase", dockMute: "Im Ladecase stumm",
        restart: "Empfänger neu starten", input: "Audioeingang",
        demo: "Demo", recording: "Diktat läuft",
        sending: "Wird gesendet …", sent: "Gesendet", muted: "Stummgeschaltet",
    },
    en: {
        notRunning: "MicDeck is not running", start: "Start MicDeck", open: "Open MicDeck",
        mute: "Mute", dockDictation: "Dictation via charging case", dockMute: "Mute in charging case",
        restart: "Restart receiver", input: "Audio input",
        demo: "Demo", recording: "Dictating",
        sending: "Sending …", sent: "Sent", muted: "Muted",
    },
};
const LANG = (GLib.get_language_names()[0] || "en").startsWith("de") ? "de" : "en";
const T = TEXT[LANG];

class MicDeckApplet extends Applet.IconApplet {
    constructor(metadata, orientation, panelHeight, instanceId) {
        super(orientation, panelHeight, instanceId);
        this._path = metadata.path;
        this._state = null;
        this._proxy = null;
        this._signals = [];
        this._sentTimer = 0;
        this._openTimer = 0;
        this._closeTimer = 0;
        this._pulseTimer = 0;
        this._byHover = false;

        this._setIcon("off");
        this.set_applet_tooltip("");

        this.menuManager = new PopupMenu.PopupMenuManager(this);
        this.menu = new Applet.AppletPopupMenu(this, orientation);
        this.menuManager.addMenu(this.menu);
        this._buildMenu();
        this.menu.connect("open-state-changed", (m, open) => {
            if (!open) this._byHover = false;
            this._stopCloseWatch();
        });

        this.actor.connect("enter-event", () => this._onEnter());
        this.actor.connect("leave-event", () => this._cancelOpen());

        this._ctxOpen = new PopupMenu.PopupIconMenuItem(T.open, "audio-input-microphone-symbolic", St.IconType.SYMBOLIC);
        this._ctxOpen.connect("activate", () => this._showWindow());
        this._applet_context_menu.addMenuItem(this._ctxOpen, 0);
        this._applet_context_menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem(), 1);

        // Tell MicDeck we are here, so it hides its own tray icon.
        this._ownId = Gio.bus_own_name(Gio.BusType.SESSION, APPLET_NAME, Gio.BusNameOwnerFlags.NONE,
            null, null, null);
        this._watchId = Gio.bus_watch_name(Gio.BusType.SESSION, BUS, Gio.BusNameWatcherFlags.NONE,
            () => this._connect(), () => this._disconnect());
        this._render();
    }

    // ---- D-Bus -------------------------------------------------------------

    _connect() {
        new MicDeckProxy(Gio.DBus.session, BUS, PATH, (proxy, error) => {
            if (error) {
                global.logError("MicDeck applet: " + error);
                return;
            }
            this._proxy = proxy;
            this._signals = [
                proxy.connectSignal("StateChanged", (p, sender, [json]) => this._onState(json)),
                proxy.connectSignal("Level", (p, sender, [db]) => this._onLevel(db)),
            ];
            proxy.StateRemote((result, err) => {
                if (!err && result) this._onState(result[0]);
            });
        });
    }

    _disconnect() {
        if (this._proxy) this._signals.forEach(id => this._proxy.disconnectSignal(id));
        this._signals = [];
        this._proxy = null;
        this._state = null;
        this._render();
    }

    _call(method, ...args) {
        if (this._proxy) this._proxy[method + "Remote"](...args, () => {});
    }

    _showWindow() {
        if (this._proxy) this._call("ShowWindow");
        else GLib.spawn_command_line_async("micdeck");
        this.menu.close();
    }

    _onState(json) {
        try {
            this._state = JSON.parse(json);
        } catch (e) {
            return;
        }
        if (this._state.dictation === "sent") {
            if (this._sentTimer) Mainloop.source_remove(this._sentTimer);
            this._sentTimer = Mainloop.timeout_add(SENT_SHOWN_MS, () => {
                this._sentTimer = 0;
                if (this._state && this._state.dictation === "sent") this._state.dictation = "idle";
                this._render();
                return false;
            });
        }
        this._render();
    }

    // ---- icon --------------------------------------------------------------

    _iconName() {
        const s = this._state;
        if (!s) return "off";
        if (s.dictation === "recording" || s.dictation === "sending" || s.dictation === "sent") return s.dictation;
        if (s.muted === true) return "muted";
        if (s.restarting) return "pending";
        if (s.receiverOff) return "off";
        return { ok: "ok", warn: "warn", error: "error", pending: "pending" }[s.level] || "off";
    }

    // Recording (dictation, or a program records from the DJI input): the
    // icon breathes instead of standing still.
    _recording() {
        const s = this._state;
        return !!s && s.muted !== true && (s.dictation === "recording" || !!s.capturing);
    }

    _updatePulse() {
        const want = this._recording();
        if (want === !!this._pulseTimer) return;
        const icon = this._applet_icon;
        if (!want) {
            Mainloop.source_remove(this._pulseTimer);
            this._pulseTimer = 0;
            if (icon) {
                icon.remove_all_transitions();
                icon.ease({ opacity: 255, duration: 200 });
            }
            return;
        }
        let dim = false;
        const step = () => {
            const target = this._applet_icon;
            if (target) {
                dim = !dim;
                target.ease({ opacity: dim ? 90 : 255, duration: PULSE_MS,
                    mode: Clutter.AnimationMode.EASE_IN_OUT_SINE });
            }
            return true;
        };
        step();
        this._pulseTimer = Mainloop.timeout_add(PULSE_MS, step);
    }

    _setIcon(name) {
        if (this._iconShown === name) return;
        this._iconShown = name;
        this.set_applet_icon_path(`${this._path}/icons/micdeck-${name}.svg`);
    }

    // ---- hover -------------------------------------------------------------

    _onEnter() {
        this._cancelOpen();
        if (this.menu.isOpen) return;
        this._openTimer = Mainloop.timeout_add(OPEN_DELAY_MS, () => {
            this._openTimer = 0;
            if (!this.menu.isOpen) {
                this._byHover = true;
                this.menu.open(true);
                this._startCloseWatch();
            }
            return false;
        });
    }

    _cancelOpen() {
        if (this._openTimer) Mainloop.source_remove(this._openTimer);
        this._openTimer = 0;
    }

    // The menu grabs the pointer while open, so enter/leave on the panel stop
    // arriving; watch the pointer position instead.
    _startCloseWatch() {
        this._stopCloseWatch();
        let outsideSince = 0;
        this._closeTimer = Mainloop.timeout_add(100, () => {
            if (!this.menu.isOpen || !this._byHover) {
                this._closeTimer = 0;
                return false;
            }
            const [x, y] = global.get_pointer();
            const inside = this._contains(this.actor, x, y) || this._contains(this.menu.actor, x, y);
            const now = GLib.get_monotonic_time() / 1000;
            if (inside) outsideSince = 0;
            else if (!outsideSince) outsideSince = now;
            else if (now - outsideSince >= CLOSE_DELAY_MS) {
                this._closeTimer = 0;
                this.menu.close(true);
                return false;
            }
            return true;
        });
    }

    _stopCloseWatch() {
        if (this._closeTimer) Mainloop.source_remove(this._closeTimer);
        this._closeTimer = 0;
    }

    _contains(actor, x, y) {
        if (!actor || !actor.mapped) return false;
        const [ax, ay] = actor.get_transformed_position();
        const [w, h] = actor.get_transformed_size();
        return x >= ax - 2 && x <= ax + w + 2 && y >= ay - 2 && y <= ay + h + 2;
    }

    on_applet_clicked() {
        // Hover shows the menu; a click opens MicDeck itself.
        this._cancelOpen();
        this._showWindow();
    }

    // ---- menu --------------------------------------------------------------

    _buildMenu() {
        this.menu.box.style = "min-width: 340px; padding: 4px 0;";

        // Header: picture, name, status line.
        // Own rows, not menu items: items share column widths with each other.
        const hbox = new St.BoxLayout({ style: "spacing: 12px; padding: 6px 14px 8px 14px;", x_expand: true });
        this._headIcon = new St.Icon({ icon_size: 44 });
        const headButton = new St.Button({ child: this._headIcon, style: "padding: 0; border-radius: 10px;",
            y_align: Clutter.ActorAlign.START });
        headButton.connect("clicked", () => this._showWindow());
        const text = new St.BoxLayout({ vertical: true, y_align: Clutter.ActorAlign.CENTER, x_expand: true, style: "width: 250px;" });
        const name = new St.Label({ text: "MicDeck", style: "font-weight: bold; font-size: 12pt;" });
        const line = new St.BoxLayout({ style: "spacing: 6px;" });
        this._dot = new St.Widget({ style: "border-radius: 4px; width: 8px; height: 8px;", y_align: Clutter.ActorAlign.CENTER });
        this._title = new St.Label({ text: "", style: "max-width: 236px;" });
        line.add_child(this._dot);
        line.add_child(this._title);
        this._detail = new St.Label({ text: "", style: "font-size: 8.5pt; color: rgba(255,255,255,0.6); width: 250px;" });
        this._detail.clutter_text.line_wrap = true;
        this._detail.clutter_text.ellipsize = 0;
        text.add_child(name);
        text.add_child(line);
        text.add_child(this._detail);
        const gear = new St.Button({ style_class: "panel-button", style: "padding: 4px; border-radius: 6px;",
            child: new St.Icon({ icon_name: "emblem-system-symbolic", icon_type: St.IconType.SYMBOLIC, icon_size: 18 }),
            y_align: Clutter.ActorAlign.START });
        gear.connect("clicked", () => this._showWindow());
        hbox.add_child(headButton);
        hbox.add_child(text);
        hbox.add_child(gear);
        this.menu.box.add_child(hbox);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        // Live level: 20 segments green → yellow → red, value on the right.
        const lbox = new St.BoxLayout({ style: "spacing: 10px; padding: 6px 14px;", x_expand: true });
        this._levelItem = { actor: lbox };
        lbox.add_child(new St.Icon({ icon_name: "audio-input-microphone-symbolic", icon_type: St.IconType.SYMBOLIC,
            icon_size: 18, y_align: Clutter.ActorAlign.CENTER }));
        const segs = new St.BoxLayout({ style: "spacing: 3px;", y_align: Clutter.ActorAlign.CENTER, x_expand: true });
        this._segs = [];
        for (let i = 0; i < SEGMENTS; i++) {
            const seg = new St.Widget({ style: "border-radius: 2px; width: 9px; height: 16px;" });
            segs.add_child(seg);
            this._segs.push(seg);
        }
        this._dbLabel = new St.Label({ text: "", style: "min-width: 52px; text-align: right; font-feature-settings: 'tnum';",
            y_align: Clutter.ActorAlign.CENTER });
        lbox.add_child(segs);
        lbox.add_child(this._dbLabel);
        this.menu.box.add_child(lbox);
        this._paintLevel(null);

        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());

        this._mute = new PopupMenu.PopupSwitchMenuItem(T.mute, false);
        this._mute.connect("toggled", (item, on) => this._call("SetMuted", on));
        this.menu.addMenuItem(this._mute);
        this._dockDictation = new PopupMenu.PopupSwitchMenuItem(T.dockDictation, false);
        this._dockDictation.connect("toggled", (item, on) => this._call("SetAppSetting", "dockDictation", on));
        this.menu.addMenuItem(this._dockDictation);
        this._dockMute = new PopupMenu.PopupSwitchMenuItem(T.dockMute, false);
        this._dockMute.connect("toggled", (item, on) => this._call("SetAppSetting", "dockMute", on));
        this.menu.addMenuItem(this._dockMute);

        this._restart = new PopupMenu.PopupIconMenuItem(T.restart, "view-refresh-symbolic", St.IconType.SYMBOLIC);
        this._restart.connect("activate", () => this._call("RestartReceiver"));
        this.menu.addMenuItem(this._restart);

        this._input = new PopupMenu.PopupIconMenuItem(T.input, "audio-card-symbolic", St.IconType.SYMBOLIC, { reactive: false });
        this._inputValue = new St.Label({ text: "", style: "color: rgba(255,255,255,0.6); max-width: 170px;" });
        this._input.addActor(this._inputValue, { align: St.Align.END });
        this.menu.addMenuItem(this._input);
    }

    _render() {
        const s = this._state;
        const icon = this._iconName();
        this._setIcon(icon);
        this._updatePulse();
        this._headIcon.gicon = Gio.FileIcon.new(Gio.File.new_for_path(`${this._path}/icons/micdeck-${icon}.svg`));

        const running = !!this._proxy;
        let title, detail, color;
        if (!running || !s) {
            title = T.notRunning;
            detail = "";
            color = COLORS.off;
        } else {
            title = { recording: T.recording, sending: T.sending, sent: T.sent }[s.dictation]
                || (s.muted ? T.muted : s.title);
            if (s.simulated) title = `${T.demo}: ${title}`;
            detail = s.dictation === "idle" || !s.dictation ? s.detail : "";
            color = s.muted ? COLORS.error : (COLORS[s.level] || COLORS.off);
        }
        this._title.text = title;
        this._detail.text = detail || "";
        this._detail.visible = !!detail;
        this._dot.style = `border-radius: 4px; width: 8px; height: 8px; background-color: ${color};`;

        this._levelItem.actor.visible = running && !!s && s.receiver;
        if (!running || !s || s.muted) this._paintLevel(null);

        const have = running && !!s;
        this._mute.actor.visible = have;
        this._dockDictation.actor.visible = have;
        this._dockMute.actor.visible = have;
        this._restart.actor.visible = have && s.receiver;
        this._input.actor.visible = have && !!s.input;
        if (have) {
            this._mute.setToggleState(s.muted === true);
            this._mute.setSensitive(s.muted !== null && s.muted !== undefined);
            this._dockDictation.setToggleState(!!s.dockDictation);
            this._dockMute.setToggleState(!!s.dockMute);
            this._inputValue.text = s.input || "";
        }
    }

    _onLevel(db) {
        if (!this.menu.isOpen || !this._state || this._state.muted) return;
        this._paintLevel(db);
    }

    _paintLevel(db) {
        const lit = db == null ? 0 : Math.round(Math.max(0, Math.min(1, (db - FLOOR_DB) / -FLOOR_DB)) * SEGMENTS);
        this._segs.forEach((seg, i) => {
            const share = (i + 1) / SEGMENTS;
            const on = share <= 0.6 ? "#3ddc68" : share <= 0.85 ? "#ffc233" : "#ff4b4b";
            const color = i < lit ? on : "rgba(255,255,255,0.12)";
            seg.style = `border-radius: 2px; width: 9px; height: 16px; background-color: ${color};`;
        });
        this._dbLabel.text = db == null || db <= -119 ? "– dB" : `${Math.round(db)} dB`;
    }

    on_applet_removed_from_panel() {
        this._cancelOpen();
        this._stopCloseWatch();
        if (this._sentTimer) Mainloop.source_remove(this._sentTimer);
        if (this._pulseTimer) Mainloop.source_remove(this._pulseTimer);
        if (this._watchId) Gio.bus_unwatch_name(this._watchId);
        if (this._ownId) Gio.bus_unown_name(this._ownId);
        this._disconnect();
    }
}

function main(metadata, orientation, panelHeight, instanceId) {
    return new MicDeckApplet(metadata, orientation, panelHeight, instanceId);
}
