//! Event-driven Wayland clipboard bundles. Every representation is read from one
//! data-control offer; replay owns one source until the compositor cancels it.
use crate::{
    Result,
    clipboard::{self, Format, History, MAX_BYTES, MAX_TOTAL},
    invalid,
};
use std::{
    collections::{HashMap, HashSet},
    env,
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::process::CommandExt,
    },
    path::Path,
    process::{Child, Command, Stdio},
    sync::Arc,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
    protocol::{wl_callback, wl_registry, wl_seat},
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as ed, ext_data_control_manager_v1 as em,
    ext_data_control_offer_v1 as eo, ext_data_control_source_v1 as es,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1 as wd, zwlr_data_control_manager_v1 as wm,
    zwlr_data_control_offer_v1 as wo, zwlr_data_control_source_v1 as ws,
};

pub const IDENTITY_MIME: &str = "application/x-desktop-foundation-clipboard";
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FORMATS: usize = 32;
const MAX_SENDS: usize = 16;
const HANDOFF_MAGIC: &[u8; 8] = b"DFCLIP01";

fn transport_error(error: impl std::fmt::Display) -> crate::Error {
    invalid(format!("Wayland clipboard: {error}"))
}

fn sensitive(mime: &str) -> bool {
    let mime = mime.to_ascii_lowercase();
    matches!(
        mime.as_str(),
        "x-kde-passwordmanagerhint"
            | "application/x-kde-passwordmanagerhint"
            | "application/x-keepassxc"
            | "application/x-keepassxc-password"
    )
}

#[derive(Clone)]
enum Offer {
    Ext(eo::ExtDataControlOfferV1),
    Wlr(wo::ZwlrDataControlOfferV1),
}
impl Offer {
    fn id(&self) -> wayland_client::backend::ObjectId {
        match self {
            Self::Ext(o) => o.id(),
            Self::Wlr(o) => o.id(),
        }
    }
    fn receive(&self, mime: String, fd: &OwnedFd) {
        match self {
            Self::Ext(o) => o.receive(mime, fd.as_fd()),
            Self::Wlr(o) => o.receive(mime, fd.as_fd()),
        }
    }
    fn destroy(&self) {
        match self {
            Self::Ext(o) => o.destroy(),
            Self::Wlr(o) => o.destroy(),
        }
    }
}
enum Manager {
    Ext(em::ExtDataControlManagerV1),
    Wlr(wm::ZwlrDataControlManagerV1),
}
enum Device {
    Ext(ed::ExtDataControlDeviceV1),
    Wlr(wd::ZwlrDataControlDeviceV1),
}
enum Source {
    Ext(es::ExtDataControlSourceV1),
    Wlr(ws::ZwlrDataControlSourceV1),
}
impl Source {
    fn offer(&self, mime: String) {
        match self {
            Self::Ext(s) => s.offer(mime),
            Self::Wlr(s) => s.offer(mime),
        }
    }
    fn destroy(&self) {
        match self {
            Self::Ext(s) => s.destroy(),
            Self::Wlr(s) => s.destroy(),
        }
    }
}
struct Seat {
    proxy: wl_seat::WlSeat,
    name: Option<String>,
    keyboard: bool,
}
struct Incoming {
    mime: String,
    pipe: File,
    data: Vec<u8>,
    done: bool,
}
struct Capture {
    streams: Vec<Incoming>,
    total: usize,
    deadline: Instant,
}
struct Outgoing {
    pipe: File,
    data: Arc<[u8]>,
    offset: usize,
    deadline: Instant,
}

#[derive(Default)]
struct Client {
    ext: Option<(u32, u32)>,
    wlr: Option<(u32, u32)>,
    seats: Vec<Seat>,
    manager: Option<Manager>,
    device: Option<Device>,
    source: Option<Source>,
    registry: Option<wl_registry::WlRegistry>,
    synced: HashSet<u64>,
    offers: HashMap<wayland_client::backend::ObjectId, Vec<String>>,
    selected: Option<Offer>,
    capture: Option<Capture>,
    sends: Vec<Outgoing>,
    payloads: Option<HashMap<String, Arc<[u8]>>>,
    finished: bool,
}
impl Client {
    fn select(&mut self, offer: Option<Offer>) {
        // Closing all pending pipes cancels an obsolete transfer immediately;
        // data from distinct selections can never be joined into one bundle.
        self.capture = None;
        if let Some(old) = self.selected.take() {
            old.destroy();
        }
        let Some(offer) = offer else {
            return;
        };
        let mimes = self.offers.remove(&offer.id()).unwrap_or_default();
        if self.payloads.is_none() && !mimes.iter().any(|m| m == IDENTITY_MIME || sensitive(m)) {
            let mut streams = Vec::new();
            for mime in mimes
                .into_iter()
                .filter(|mime| clipboard::supported_mime(mime))
                .take(MAX_FORMATS)
            {
                match pipe() {
                    Ok((reader, writer)) => {
                        offer.receive(mime.clone(), &writer);
                        streams.push(Incoming {
                            mime,
                            pipe: File::from(reader),
                            data: Vec::new(),
                            done: false,
                        });
                    }
                    Err(error) => {
                        eprintln!("Clipboard transfer could not start: {error}");
                        break;
                    }
                }
            }
            if !streams.is_empty() {
                self.capture = Some(Capture {
                    streams,
                    total: 0,
                    deadline: Instant::now() + TRANSFER_TIMEOUT,
                });
            }
        }
        self.selected = Some(offer);
    }
    fn mime(&mut self, id: wayland_client::backend::ObjectId, mime: String) {
        let mime = if sensitive(&mime) {
            mime.to_ascii_lowercase()
        } else {
            mime
        };
        let mimes = self.offers.entry(id).or_default();
        // Retain privacy/replay markers even when a malicious source exhausts
        // the supported-format limit before advertising its hint.
        if ((mimes.len() < MAX_FORMATS && clipboard::supported_mime(&mime))
            || sensitive(&mime)
            || mime == IDENTITY_MIME)
            && !mimes.contains(&mime)
        {
            mimes.push(mime);
        }
    }
    fn send(&mut self, mime: &str, fd: OwnedFd) {
        if self.sends.len() >= MAX_SENDS {
            return;
        }
        let Some(data) = self.payloads.as_ref().and_then(|p| p.get(mime)).cloned() else {
            return;
        };
        if nonblocking(fd.as_raw_fd()).is_err() {
            return;
        }
        self.sends.push(Outgoing {
            pipe: File::from(fd),
            data,
            offset: 0,
            deadline: Instant::now() + TRANSFER_TIMEOUT,
        });
    }
    fn advance_transfers(&mut self) {
        if let Some(capture) = &mut self.capture {
            let mut discard = Instant::now() >= capture.deadline;
            for stream in &mut capture.streams {
                if stream.done {
                    continue;
                }
                let mut buffer = [0u8; 16384];
                loop {
                    match stream.pipe.read(&mut buffer) {
                        Ok(0) => {
                            stream.done = true;
                            break;
                        }
                        Ok(n) => {
                            if stream.data.len().saturating_add(n) > MAX_BYTES
                                || capture.total.saturating_add(n) > MAX_TOTAL
                            {
                                discard = true;
                                break;
                            }
                            capture.total += n;
                            stream.data.extend_from_slice(&buffer[..n]);
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => {
                            discard = true;
                            break;
                        }
                    }
                }
                if discard {
                    break;
                }
            }
            if discard {
                self.capture = None;
            }
        }
        self.sends.retain_mut(|send| {
            if Instant::now() >= send.deadline {
                return false;
            }
            while send.offset < send.data.len() {
                match send.pipe.write(&send.data[send.offset..]) {
                    Ok(0) => return false,
                    Ok(n) => send.offset += n,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => return true,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => return false,
                }
            }
            false
        });
    }
    fn take_bundle(&mut self) -> Option<Vec<Format>> {
        if self
            .capture
            .as_ref()
            .is_some_and(|c| c.streams.iter().all(|s| s.done))
        {
            Some(
                self.capture
                    .take()
                    .unwrap()
                    .streams
                    .into_iter()
                    .filter(|s| !s.data.is_empty())
                    .map(|s| Format {
                        mime: s.mime,
                        data: s.data,
                    })
                    .collect(),
            )
        } else {
            None
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Client {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "ext_data_control_manager_v1" => state.ext = Some((name, version)),
                "zwlr_data_control_manager_v1" => state.wlr = Some((name, version)),
                "wl_seat" => {
                    let proxy =
                        registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(7), qh, ());
                    state.seats.push(Seat {
                        proxy,
                        name: None,
                        keyboard: false,
                    });
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { name } => {
                if state.ext.is_some_and(|(n, _)| n == name) {
                    state.ext = None;
                }
                if state.wlr.is_some_and(|(n, _)| n == name) {
                    state.wlr = None;
                }
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for Client {
    fn event(
        state: &mut Self,
        proxy: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let Some(seat) = state.seats.iter_mut().find(|s| &s.proxy == proxy) {
            match event {
                wl_seat::Event::Name { name } => seat.name = Some(name),
                wl_seat::Event::Capabilities { capabilities } => {
                    seat.keyboard = capabilities
                        .into_result()
                        .is_ok_and(|c| c.contains(wl_seat::Capability::Keyboard))
                }
                _ => {}
            }
        }
    }
}
impl Dispatch<wl_callback::WlCallback, u64> for Client {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        serial: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if matches!(event, wl_callback::Event::Done { .. }) {
            state.synced.insert(*serial);
        }
    }
}
wayland_client::delegate_noop!(Client: em::ExtDataControlManagerV1);
wayland_client::delegate_noop!(Client: wm::ZwlrDataControlManagerV1);

macro_rules! device_dispatch {
    ($module:ident, $ty:ident, $offer_module:ident, $offer_ty:ident, $variant:ident) => {
        impl Dispatch<$module::$ty, ()> for Client {
            fn event(state: &mut Self, _: &$module::$ty, event: $module::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
                match event {
                    $module::Event::Selection { id } => state.select(id.map(Offer::$variant)),
                    $module::Event::PrimarySelection { id: Some(offer) } => { state.offers.remove(&offer.id()); offer.destroy(); }
                    $module::Event::Finished => state.finished = true,
                    _ => {}
                }
            }
            wayland_client::event_created_child!(Client, $module::$ty, [0 => ($offer_module::$offer_ty, ())]);
        }
    };
}
device_dispatch!(ed, ExtDataControlDeviceV1, eo, ExtDataControlOfferV1, Ext);
device_dispatch!(wd, ZwlrDataControlDeviceV1, wo, ZwlrDataControlOfferV1, Wlr);
macro_rules! offer_dispatch {
    ($module:ident, $ty:ident) => {
        impl Dispatch<$module::$ty, ()> for Client {
            fn event(
                state: &mut Self,
                proxy: &$module::$ty,
                event: $module::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                if let $module::Event::Offer { mime_type } = event {
                    state.mime(proxy.id(), mime_type);
                }
            }
        }
    };
}
offer_dispatch!(eo, ExtDataControlOfferV1);
offer_dispatch!(wo, ZwlrDataControlOfferV1);
macro_rules! source_dispatch {
    ($module:ident, $ty:ident) => {
        impl Dispatch<$module::$ty, ()> for Client {
            fn event(
                state: &mut Self,
                _: &$module::$ty,
                event: $module::Event,
                _: &(),
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
                match event {
                    $module::Event::Send { mime_type, fd } => state.send(&mime_type, fd),
                    $module::Event::Cancelled => {
                        state.finished = true;
                        if let Some(source) = state.source.take() {
                            source.destroy();
                        }
                    }
                    _ => {}
                }
            }
        }
    };
}
source_dispatch!(es, ExtDataControlSourceV1);
source_dispatch!(ws, ZwlrDataControlSourceV1);

fn nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: fcntl operates only on a valid, borrowed descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [-1; 2];
    // SAFETY: the array has space for the two returned descriptors. O_CLOEXEC
    // prevents transfer pipes from leaking into unrelated application launches.
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let (read, write) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    nonblocking(read.as_raw_fd())?;
    Ok((read, write))
}
fn timeout_ms(deadline: Option<Instant>) -> i32 {
    deadline.map_or(-1, |d| {
        d.saturating_duration_since(Instant::now())
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32
    })
}
fn poll(fds: &mut [libc::pollfd], deadline: Option<Instant>) -> io::Result<()> {
    loop {
        let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, timeout_ms(deadline)) };
        if result >= 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}
fn pump(
    conn: &Connection,
    queue: &mut EventQueue<Client>,
    state: &mut Client,
    deadline: Option<Instant>,
) -> Result<()> {
    let dispatched = queue.dispatch_pending(state).map_err(transport_error)?;
    state.advance_transfers();
    if dispatched != 0
        || state.finished
        || state
            .capture
            .as_ref()
            .is_some_and(|c| c.streams.iter().all(|s| s.done))
    {
        return Ok(());
    }
    let Some(guard) = queue.prepare_read() else {
        return Ok(());
    };
    let blocked_write = match conn.flush() {
        Ok(()) => false,
        Err(wayland_client::backend::WaylandError::Io(e))
            if e.kind() == io::ErrorKind::WouldBlock =>
        {
            true
        }
        Err(error) => return Err(transport_error(error)),
    };
    let mut fds = vec![libc::pollfd {
        fd: conn.as_fd().as_raw_fd(),
        events: libc::POLLIN | if blocked_write { libc::POLLOUT } else { 0 },
        revents: 0,
    }];
    let mut wakeup = deadline;
    if let Some(capture) = &state.capture {
        wakeup = Some(wakeup.map_or(capture.deadline, |d| d.min(capture.deadline)));
        fds.extend(
            capture
                .streams
                .iter()
                .filter(|s| !s.done)
                .map(|s| libc::pollfd {
                    fd: s.pipe.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                }),
        );
    }
    for send in &state.sends {
        wakeup = Some(wakeup.map_or(send.deadline, |d| d.min(send.deadline)));
        fds.push(libc::pollfd {
            fd: send.pipe.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        });
    }
    poll(&mut fds, wakeup)?;
    if fds[0].revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) != 0 {
        guard.read().map_err(transport_error)?;
    } else {
        drop(guard);
    }
    // Selection replacements are dispatched before pipe completion, so stale
    // transfers are discarded instead of briefly entering history.
    queue.dispatch_pending(state).map_err(transport_error)?;
    state.advance_transfers();
    Ok(())
}
fn sync(
    conn: &Connection,
    queue: &mut EventQueue<Client>,
    state: &mut Client,
    serial: u64,
    deadline: Instant,
) -> Result<()> {
    conn.display().sync(&queue.handle(), serial);
    while !state.synced.remove(&serial) {
        if state.finished {
            return Err(invalid("Clipboard ownership was cancelled"));
        }
        if Instant::now() >= deadline {
            return Err(invalid("Wayland clipboard handshake timed out"));
        }
        if state.payloads.is_some() {
            state.take_bundle();
        }
        pump(conn, queue, state, Some(deadline))?;
    }
    Ok(())
}
fn connect(deadline: Instant) -> Result<(Connection, EventQueue<Client>, Client)> {
    let conn = Connection::connect_to_env().map_err(transport_error)?;
    let mut queue = conn.new_event_queue();
    let mut state = Client {
        registry: Some(conn.display().get_registry(&queue.handle(), ())),
        ..Client::default()
    };
    sync(&conn, &mut queue, &mut state, 1, deadline)?;
    // Seat names/capabilities arrive in response to binds issued by the first
    // registry dispatch; a second bounded sync lets us choose the default seat.
    sync(&conn, &mut queue, &mut state, 2, deadline)?;
    let requested = env::var("DF_CLIPBOARD_SEAT").ok().filter(|s| !s.is_empty());
    let seat = if let Some(name) = requested {
        state
            .seats
            .iter()
            .find(|s| s.name.as_deref() == Some(&name))
    } else {
        state
            .seats
            .iter()
            .find(|s| s.keyboard)
            .or_else(|| state.seats.first())
    }
    .ok_or_else(|| invalid("No matching Wayland seat for clipboard"))?
    .proxy
    .clone();
    let registry = state.registry.as_ref().unwrap();
    let qh = queue.handle();
    if let Some((name, _)) = state.ext {
        let manager = registry.bind::<em::ExtDataControlManagerV1, _, _>(name, 1, &qh, ());
        state.device = Some(Device::Ext(manager.get_data_device(&seat, &qh, ())));
        state.manager = Some(Manager::Ext(manager));
    } else if let Some((name, version)) = state.wlr {
        let manager =
            registry.bind::<wm::ZwlrDataControlManagerV1, _, _>(name, version.min(2), &qh, ());
        state.device = Some(Device::Wlr(manager.get_data_device(&seat, &qh, ())));
        state.manager = Some(Manager::Wlr(manager));
    } else {
        return Err(invalid(
            "The compositor does not expose Wayland clipboard data-control",
        ));
    }
    Ok((conn, queue, state))
}

/// Capture one bundle per regular-clipboard offer, sleeping in poll when idle.
/// History locks exist only during the short SQLite commit/export operation.
pub fn watch(directory: &Path) -> Result<()> {
    let (conn, mut queue, mut state) = connect(Instant::now() + TRANSFER_TIMEOUT)?;
    while !state.finished {
        pump(&conn, &mut queue, &mut state, None)?;
        if let Some(formats) = state.take_bundle().filter(|f| !f.is_empty())
            && let Err(error) = History::open(directory).and_then(|mut h| h.store_bundle(&formats))
        {
            eprintln!("Clipboard item could not be saved: {error}");
        }
    }
    Err(invalid("Wayland clipboard device was removed"))
}

fn validate_formats(formats: &[Format], identity: &str) -> Result<()> {
    if !clipboard::valid_id(identity) || formats.is_empty() || formats.len() > MAX_FORMATS {
        return Err(invalid("Invalid clipboard replay bundle"));
    }
    let mut total = 0usize;
    let mut seen = HashSet::new();
    for format in formats {
        if !clipboard::supported_mime(&format.mime)
            || format.mime.len() > 1024
            || format.data.is_empty()
            || format.data.len() > MAX_BYTES
            || !seen.insert(&format.mime)
        {
            return Err(invalid("Invalid clipboard replay format"));
        }
        total = total.saturating_add(format.data.len());
        if total > MAX_TOTAL {
            return Err(invalid("Clipboard replay bundle is too large"));
        }
    }
    Ok(())
}
fn write_handoff(output: &mut impl Write, formats: &[Format], identity: &str) -> Result<()> {
    validate_formats(formats, identity)?;
    output.write_all(HANDOFF_MAGIC)?;
    output.write_all(identity.as_bytes())?;
    output.write_all(&(formats.len() as u32).to_le_bytes())?;
    for format in formats {
        output.write_all(&(format.mime.len() as u32).to_le_bytes())?;
        output.write_all(&(format.data.len() as u32).to_le_bytes())?;
        output.write_all(format.mime.as_bytes())?;
        output.write_all(&format.data)?;
    }
    Ok(())
}
fn read_handoff(input: &mut impl Read) -> Result<(Vec<Format>, String)> {
    let mut magic = [0; 8];
    input.read_exact(&mut magic)?;
    if &magic != HANDOFF_MAGIC {
        return Err(invalid("Invalid clipboard publisher handoff"));
    }
    let mut identity = [0; 64];
    input.read_exact(&mut identity)?;
    let identity =
        String::from_utf8(identity.to_vec()).map_err(|_| invalid("Invalid clipboard identity"))?;
    let mut size = [0; 4];
    input.read_exact(&mut size)?;
    let count = u32::from_le_bytes(size) as usize;
    if count == 0 || count > MAX_FORMATS {
        return Err(invalid("Invalid clipboard format count"));
    }
    let mut formats = Vec::with_capacity(count);
    let mut total = 0usize;
    for _ in 0..count {
        input.read_exact(&mut size)?;
        let mime_len = u32::from_le_bytes(size) as usize;
        input.read_exact(&mut size)?;
        let data_len = u32::from_le_bytes(size) as usize;
        total = total.saturating_add(data_len);
        if mime_len == 0 || mime_len > 1024 || data_len > MAX_BYTES || total > MAX_TOTAL {
            return Err(invalid("Clipboard publisher handoff exceeds limits"));
        }
        let mut mime = vec![0; mime_len];
        input.read_exact(&mut mime)?;
        let mut data = vec![0; data_len];
        input.read_exact(&mut data)?;
        formats.push(Format {
            mime: String::from_utf8(mime).map_err(|_| invalid("Invalid clipboard MIME"))?,
            data,
        });
    }
    validate_formats(&formats, &identity)?;
    Ok((formats, identity))
}
struct TimedFd {
    file: File,
    deadline: Instant,
}
impl Read for TimedFd {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.file.read(bytes) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => self.wait(libc::POLLIN)?,
                result => return result,
            }
        }
    }
}
impl Write for TimedFd {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            match self.file.write(bytes) {
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => self.wait(libc::POLLOUT)?,
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl TimedFd {
    fn new(fd: OwnedFd, deadline: Instant) -> io::Result<Self> {
        nonblocking(fd.as_raw_fd())?;
        Ok(Self {
            file: File::from(fd),
            deadline,
        })
    }
    fn wait(&self, events: i16) -> io::Result<()> {
        if Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Clipboard publisher handoff timed out",
            ));
        }
        poll(
            &mut [libc::pollfd {
                fd: self.file.as_raw_fd(),
                events,
                revents: 0,
            }],
            Some(self.deadline),
        )
    }
}

/// Publish through a short handoff to an ownership-only child. The child is
/// detached before any Wayland connection, starts no threads, and confirms the
/// compositor processed set_selection before the caller can report success.
pub fn publish(formats: Vec<Format>, identity: &str) -> Result<()> {
    publish_with_executable(&env::current_exe()?, formats, identity)
}
struct PublisherGroup {
    child: Child,
    confirmed: bool,
}
impl Drop for PublisherGroup {
    fn drop(&mut self) {
        if !self.confirmed {
            // SAFETY: pre_exec creates this isolated process group. Keeping the
            // intermediate child unreaped until here reserves its ID, so an
            // unrelated process group cannot acquire it during a failed handoff.
            unsafe {
                libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL);
            }
        }
        let _ = self.child.wait();
    }
}
fn publish_with_executable(executable: &Path, formats: Vec<Format>, identity: &str) -> Result<()> {
    validate_formats(&formats, identity)?;
    let mut command = Command::new(executable);
    command
        .args(["clipboard", "serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // SAFETY: pre_exec executes only async-signal-safe system calls. The
    // intermediate child is reaped here; the final owner is adopted by init.
    // No connection, SQLite lock, or client thread crosses this fork boundary.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            let pid = libc::fork();
            if pid < 0 {
                return Err(io::Error::last_os_error());
            }
            if pid > 0 {
                libc::_exit(0);
            }
            Ok(())
        });
    }
    let mut group = PublisherGroup {
        child: command.spawn()?,
        confirmed: false,
    };
    let deadline = Instant::now() + TRANSFER_TIMEOUT;
    let mut input = TimedFd::new(group.child.stdin.take().unwrap().into(), deadline)?;
    let mut output = TimedFd::new(group.child.stdout.take().unwrap().into(), deadline)?;
    write_handoff(&mut input, &formats, identity)?;
    drop(input);
    let mut ready = [0; 6];
    output
        .read_exact(&mut ready)
        .map_err(|error| invalid(format!("Clipboard ownership was not confirmed: {error}")))?;
    if &ready != b"READY\n" {
        return Err(invalid("Clipboard ownership was not confirmed"));
    }
    if !group.child.wait()?.success() {
        return Err(invalid("Clipboard publisher could not detach"));
    }
    group.confirmed = true;
    Ok(())
}

/// Internal ownership process; stdin contains only the bounded replay bundle.
pub fn serve_stdin() -> Result<()> {
    let deadline = Instant::now() + TRANSFER_TIMEOUT;
    // SAFETY: dup returns a new owned descriptor without closing caller stdin.
    let fd = unsafe { libc::fcntl(libc::STDIN_FILENO, libc::F_DUPFD_CLOEXEC, 3) };
    if fd < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let mut input = TimedFd::new(unsafe { OwnedFd::from_raw_fd(fd) }, deadline)?;
    let (formats, identity) = read_handoff(&mut input)?;
    drop(input);
    serve(formats, identity, &mut io::stdout(), deadline)
}
fn serve(
    formats: Vec<Format>,
    identity: String,
    ready: &mut impl Write,
    deadline: Instant,
) -> Result<()> {
    let (conn, mut queue, mut state) = connect(deadline)?;
    let mut payloads: HashMap<String, Arc<[u8]>> = formats
        .into_iter()
        .map(|f| (f.mime, Arc::from(f.data)))
        .collect();
    payloads.insert(IDENTITY_MIME.into(), Arc::from(identity.into_bytes()));
    state.payloads = Some(payloads);
    let qh = queue.handle();
    let source = match state.manager.as_ref().unwrap() {
        Manager::Ext(manager) => Source::Ext(manager.create_data_source(&qh, ())),
        Manager::Wlr(manager) => Source::Wlr(manager.create_data_source(&qh, ())),
    };
    for mime in state.payloads.as_ref().unwrap().keys() {
        source.offer(mime.clone());
    }
    match (&state.device, &source) {
        (Some(Device::Ext(device)), Source::Ext(source)) => device.set_selection(Some(source)),
        (Some(Device::Wlr(device)), Source::Wlr(source)) => device.set_selection(Some(source)),
        _ => unreachable!(),
    }
    state.source = Some(source);
    sync(&conn, &mut queue, &mut state, 3, deadline)?;
    if state.finished {
        return Err(invalid("Clipboard ownership was cancelled before handoff"));
    }
    ready.write_all(b"READY\n")?;
    ready.flush()?;
    while !state.finished {
        pump(&conn, &mut queue, &mut state, None)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_rejects_oversized_fields_before_reading_or_allocating_payloads() {
        let id = "1".repeat(64);
        let mut header = HANDOFF_MAGIC.to_vec();
        header.extend_from_slice(id.as_bytes());
        header.extend_from_slice(&1u32.to_le_bytes());
        header.extend_from_slice(&20u32.to_le_bytes());
        header.extend_from_slice(&((MAX_BYTES + 1) as u32).to_le_bytes());
        assert!(
            read_handoff(&mut header.as_slice())
                .unwrap_err()
                .to_string()
                .contains("exceeds limits")
        );
        let mut many = HANDOFF_MAGIC.to_vec();
        many.extend_from_slice(id.as_bytes());
        many.extend_from_slice(&((MAX_FORMATS + 1) as u32).to_le_bytes());
        assert!(
            read_handoff(&mut many.as_slice())
                .unwrap_err()
                .to_string()
                .contains("format count")
        );
    }

    #[test]
    fn publisher_handoff_preserves_binary_and_rich_payloads_without_json_expansion() {
        let formats = vec![
            Format {
                mime: "text/html".into(),
                data: b"<b>Rich text</b>".to_vec(),
            },
            Format {
                mime: "image/png".into(),
                data: b"\x89PNG\r\n\x1a\n\0\xff".to_vec(),
            },
        ];
        let identity = "a".repeat(64);
        let mut encoded = Vec::new();
        write_handoff(&mut encoded, &formats, &identity).unwrap();
        let (decoded, id) = read_handoff(&mut encoded.as_slice()).unwrap();
        assert_eq!(identity, id);
        for (original, replay) in formats.iter().zip(decoded) {
            assert_eq!(original.mime, replay.mime);
            assert_eq!(original.data, replay.data);
        }
        let duplicate = vec![formats[0].clone(), formats[0].clone()];
        assert!(validate_formats(&duplicate, &identity).is_err());
    }
}
