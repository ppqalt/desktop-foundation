//! Native protocol tests use their own Unix socket and fake compositor. No test
//! connects to the desktop, reads its clipboard, or touches its state files.
use desktop_foundationctl::clipboard::{Format, History};
use desktop_foundationctl::clipboard_transport::IDENTITY_MIME;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsFd, AsRawFd, OwnedFd},
        unix::net::UnixListener,
    },
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use wayland_protocols::ext::data_control::v1::server::{
    ext_data_control_device_v1 as ed, ext_data_control_manager_v1 as em,
    ext_data_control_offer_v1 as eo, ext_data_control_source_v1 as es,
};
use wayland_protocols_wlr::data_control::v1::server::{
    zwlr_data_control_device_v1 as wd, zwlr_data_control_manager_v1 as wm,
    zwlr_data_control_offer_v1 as wo, zwlr_data_control_source_v1 as ws,
};
use wayland_server::{
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New, Resource,
    protocol::wl_seat,
};

#[derive(Clone)]
enum Device {
    Ext(ed::ExtDataControlDeviceV1),
    Wlr(wd::ZwlrDataControlDeviceV1),
}
#[derive(Clone)]
enum Source {
    Ext(es::ExtDataControlSourceV1),
    Wlr(ws::ZwlrDataControlSourceV1),
}
impl Source {
    fn send(&self, mime: String, fd: &OwnedFd) {
        match self {
            Self::Ext(s) => s.send(mime, fd.as_fd()),
            Self::Wlr(s) => s.send(mime, fd.as_fd()),
        }
    }
    fn cancel(&self) {
        match self {
            Self::Ext(s) => s.cancelled(),
            Self::Wlr(s) => s.cancelled(),
        }
    }
    fn formats(&self) -> Vec<String> {
        match self {
            Self::Ext(s) => s
                .data::<Mutex<Vec<String>>>()
                .unwrap()
                .lock()
                .unwrap()
                .clone(),
            Self::Wlr(s) => s
                .data::<Mutex<Vec<String>>>()
                .unwrap()
                .lock()
                .unwrap()
                .clone(),
        }
    }
}
#[derive(Clone)]
enum Content {
    Bytes(HashMap<String, Vec<u8>>, Option<String>),
    Relay(Source),
}
impl Content {
    fn formats(&self) -> Vec<String> {
        match self {
            Self::Bytes(data, _) => data.keys().cloned().collect(),
            Self::Relay(source) => source.formats(),
        }
    }
}
enum ServerCommand {
    Replace(Content),
    Read(String, mpsc::Sender<Vec<u8>>),
    Cancel,
    Pause(Duration, mpsc::Sender<()>),
    Stop,
}
struct Server {
    devices: Vec<Device>,
    content: Option<Content>,
    held: Vec<OwnedFd>,
    binds: Arc<AtomicUsize>,
    receives: Arc<AtomicUsize>,
}
impl Server {
    fn offer(&self, device: &Device, handle: &DisplayHandle) {
        match device {
            Device::Ext(device) => {
                if let Some(content) = &self.content {
                    let offer = device
                        .client()
                        .unwrap()
                        .create_resource::<eo::ExtDataControlOfferV1, _, Self>(
                            handle,
                            1,
                            content.clone(),
                        )
                        .unwrap();
                    device.data_offer(&offer);
                    for mime in content.formats() {
                        offer.offer(mime);
                    }
                    device.selection(Some(&offer));
                } else {
                    device.selection(None);
                }
            }
            Device::Wlr(device) => {
                if let Some(content) = &self.content {
                    let offer = device
                        .client()
                        .unwrap()
                        .create_resource::<wo::ZwlrDataControlOfferV1, _, Self>(
                            handle,
                            1,
                            content.clone(),
                        )
                        .unwrap();
                    device.data_offer(&offer);
                    for mime in content.formats() {
                        offer.offer(mime);
                    }
                    device.selection(Some(&offer));
                } else {
                    device.selection(None);
                }
            }
        }
    }
    fn replace(&mut self, content: Option<Content>, handle: &DisplayHandle) {
        if let Some(Content::Relay(old)) = self.content.take() {
            old.cancel();
        }
        self.content = content;
        for device in &self.devices {
            if match device {
                Device::Ext(d) => d.is_alive(),
                Device::Wlr(d) => d.is_alive(),
            } {
                self.offer(device, handle);
            }
        }
    }
    fn receive(&mut self, mime: String, fd: OwnedFd, content: &Content) {
        self.receives.fetch_add(1, Ordering::SeqCst);
        match content {
            Content::Relay(source) => source.send(mime, &fd),
            Content::Bytes(payloads, stall) => {
                if stall.as_deref() == Some(&mime) {
                    self.held.push(fd);
                } else if let Some(data) = payloads.get(&mime) {
                    let data = data.clone();
                    thread::spawn(move || {
                        let _ = File::from(fd).write_all(&data);
                    });
                }
            }
        }
    }
}
impl GlobalDispatch<wl_seat::WlSeat, ()> for Server {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<wl_seat::WlSeat>,
        _: &(),
        data: &mut DataInit<'_, Self>,
    ) {
        let seat = data.init(resource, ());
        seat.name("seat0".into());
        seat.capabilities(wl_seat::Capability::Keyboard);
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_seat::WlSeat,
        _: wl_seat::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
macro_rules! server_protocol {
    ($m:ident,$mt:ident,$d:ident,$dt:ident,$o:ident,$ot:ident,$s:ident,$st:ident,$variant:ident) => {
        impl GlobalDispatch<$m::$mt, ()> for Server {
            fn bind(
                _: &mut Self,
                _: &DisplayHandle,
                _: &Client,
                resource: New<$m::$mt>,
                _: &(),
                data: &mut DataInit<'_, Self>,
            ) {
                data.init(resource, ());
            }
        }
        impl Dispatch<$m::$mt, ()> for Server {
            fn request(
                state: &mut Self,
                _: &Client,
                _: &$m::$mt,
                request: $m::Request,
                _: &(),
                handle: &DisplayHandle,
                data: &mut DataInit<'_, Self>,
            ) {
                match request {
                    $m::Request::GetDataDevice { id, .. } => {
                        let device = Device::$variant(data.init(id, ()));
                        state.offer(&device, handle);
                        state.devices.push(device);
                        state.binds.fetch_add(1, Ordering::SeqCst);
                    }
                    $m::Request::CreateDataSource { id } => {
                        data.init(id, Mutex::<Vec<String>>::default());
                    }
                    _ => {}
                }
            }
        }
        impl Dispatch<$d::$dt, ()> for Server {
            fn request(
                state: &mut Self,
                _: &Client,
                _: &$d::$dt,
                request: $d::Request,
                _: &(),
                handle: &DisplayHandle,
                _: &mut DataInit<'_, Self>,
            ) {
                if let $d::Request::SetSelection { source } = request {
                    state.replace(source.map(Source::$variant).map(Content::Relay), handle);
                }
            }
        }
        impl Dispatch<$o::$ot, Content> for Server {
            fn request(
                state: &mut Self,
                _: &Client,
                _: &$o::$ot,
                request: $o::Request,
                content: &Content,
                _: &DisplayHandle,
                _: &mut DataInit<'_, Self>,
            ) {
                if let $o::Request::Receive { mime_type, fd } = request {
                    state.receive(mime_type, fd, content);
                }
            }
        }
        impl Dispatch<$s::$st, Mutex<Vec<String>>> for Server {
            fn request(
                _: &mut Self,
                _: &Client,
                _: &$s::$st,
                request: $s::Request,
                formats: &Mutex<Vec<String>>,
                _: &DisplayHandle,
                _: &mut DataInit<'_, Self>,
            ) {
                if let $s::Request::Offer { mime_type } = request {
                    formats.lock().unwrap().push(mime_type);
                }
            }
        }
    };
}
server_protocol!(
    em,
    ExtDataControlManagerV1,
    ed,
    ExtDataControlDeviceV1,
    eo,
    ExtDataControlOfferV1,
    es,
    ExtDataControlSourceV1,
    Ext
);
server_protocol!(
    wm,
    ZwlrDataControlManagerV1,
    wd,
    ZwlrDataControlDeviceV1,
    wo,
    ZwlrDataControlOfferV1,
    ws,
    ZwlrDataControlSourceV1,
    Wlr
);

struct Fixture {
    directory: PathBuf,
    socket: PathBuf,
    command: mpsc::Sender<ServerCommand>,
    binds: Arc<AtomicUsize>,
    receives: Arc<AtomicUsize>,
    thread: Option<thread::JoinHandle<()>>,
}
static NEXT: AtomicUsize = AtomicUsize::new(0);
impl Fixture {
    fn new(ext: bool) -> Self {
        Self::with_protocol(Some(ext))
    }
    fn with_protocol(protocol: Option<bool>) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "df-clipboard-transport-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&directory).unwrap();
        let socket = directory.join("private-wayland");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let (sender, receiver) = mpsc::channel();
        let binds = Arc::new(AtomicUsize::new(0));
        let receives = Arc::new(AtomicUsize::new(0));
        let mut state = Server {
            devices: Vec::new(),
            content: None,
            held: Vec::new(),
            binds: binds.clone(),
            receives: receives.clone(),
        };
        let thread = thread::spawn(move || {
            let mut display = Display::<Server>::new().unwrap();
            let mut handle = display.handle();
            handle.create_global::<Server, wl_seat::WlSeat, _>(7, ());
            if protocol == Some(true) {
                handle.create_global::<Server, em::ExtDataControlManagerV1, _>(1, ());
            } else if protocol == Some(false) {
                handle.create_global::<Server, wm::ZwlrDataControlManagerV1, _>(2, ());
            }
            loop {
                while let Ok((stream, _)) = listener.accept() {
                    handle.insert_client(stream, Arc::new(())).unwrap();
                }
                while let Ok(command) = receiver.try_recv() {
                    match command {
                        ServerCommand::Replace(content) => state.replace(Some(content), &handle),
                        ServerCommand::Cancel => state.replace(None, &handle),
                        ServerCommand::Pause(duration, ready) => {
                            ready.send(()).unwrap();
                            thread::sleep(duration);
                        }
                        ServerCommand::Stop => return,
                        ServerCommand::Read(mime, result) => {
                            let Some(Content::Relay(source)) = &state.content else {
                                panic!("No published selection");
                            };
                            let (read, write) = std::os::unix::net::UnixStream::pair().unwrap();
                            source.send(mime, &OwnedFd::from(write));
                            thread::spawn(move || {
                                let mut bytes = Vec::new();
                                let mut read = read;
                                read.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                                read.read_to_end(&mut bytes).unwrap();
                                result.send(bytes).unwrap();
                            });
                        }
                    }
                }
                display.dispatch_clients(&mut state).unwrap();
                display.flush_clients().unwrap();
                let mut fds = [
                    libc::pollfd {
                        fd: display.as_fd().as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    },
                    libc::pollfd {
                        fd: listener.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    },
                ];
                unsafe {
                    libc::poll(fds.as_mut_ptr(), 2, 5);
                }
            }
        });
        Self {
            directory,
            socket,
            command: sender,
            binds,
            receives,
            thread: Some(thread),
        }
    }
    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_desktop-foundationctl"));
        command
            .args(args)
            .env_remove("WAYLAND_SOCKET")
            .env("WAYLAND_DISPLAY", &self.socket)
            .env("XDG_RUNTIME_DIR", &self.directory)
            .env_remove("DF_CLIPBOARD_SEAT")
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }
    fn watch(&self) -> Child {
        self.command(&[
            "clipboard",
            "--state",
            self.directory.join("history").to_str().unwrap(),
            "watch",
        ])
        .spawn()
        .unwrap()
    }
    fn replace(&self, formats: Vec<Format>, stall: Option<&str>) {
        self.command
            .send(ServerCommand::Replace(Content::Bytes(
                formats.into_iter().map(|f| (f.mime, f.data)).collect(),
                stall.map(str::to_owned),
            )))
            .unwrap();
    }
    fn read(&self, mime: &str) -> Vec<u8> {
        let (tx, rx) = mpsc::channel();
        self.command
            .send(ServerCommand::Read(mime.into(), tx))
            .unwrap();
        rx.recv_timeout(Duration::from_secs(4)).unwrap()
    }
    fn index(&self) -> Option<serde_json::Value> {
        fs::read(self.directory.join("history/index.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.command.send(ServerCommand::Stop);
        self.thread.take().unwrap().join().unwrap();
        let _ = fs::remove_dir_all(&self.directory);
    }
}
fn eventually(mut ready: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(4);
    while !ready() {
        assert!(
            Instant::now() < deadline,
            "private clipboard test timed out"
        );
        thread::sleep(Duration::from_millis(5));
    }
}
fn format(mime: &str, data: &[u8]) -> Format {
    Format {
        mime: mime.into(),
        data: data.into(),
    }
}
fn stop(mut watcher: Child) {
    let _ = watcher.kill();
    watcher.wait().unwrap();
}

#[test]
fn native_ext_and_wlr_publish_all_formats_and_survive_copy_caller() {
    for ext in [true, false] {
        let fixture = Fixture::new(ext);
        let file = fixture.directory.join("example.toml");
        fs::write(&file, b"enabled = true").unwrap();
        let folder = fixture.directory.join("folder");
        fs::create_dir(&folder).unwrap();
        let uri = format!(
            "file://{}\r\nfile://{}\r\n",
            file.display(),
            folder.display()
        );
        let formats = vec![
            format("text/plain;charset=utf-8", b"formatted text"),
            format("text/html", b"<b>formatted text</b>"),
            // Exceeds pipe capacity to exercise native backpressure rather
            // than a single small write to the paste receiver.
            format(
                "text/rtf",
                format!("{{\\rtf1 {}}}", "formatted text ".repeat(32768)).as_bytes(),
            ),
            format("text/uri-list", uri.as_bytes()),
        ];
        let state = fixture.directory.join("history");
        let id = {
            let mut history = History::open(&state).unwrap();
            history.store_bundle(&formats).unwrap();
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(state.join("index.json")).unwrap(),
            )
            .unwrap()[0]["id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let output = fixture
            .command(&["clipboard", "--state", state.to_str().unwrap(), "copy", &id])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "native copy should confirm ownership"
        );
        // The one-shot command has exited; a native owner must still send every
        // requested representation, including rich text and multiple files.
        for item in &formats {
            assert_eq!(fixture.read(&item.mime), item.data);
        }
        assert_eq!(fixture.read(IDENTITY_MIME), id.as_bytes());
        fixture.command.send(ServerCommand::Cancel).unwrap();
    }
}

#[test]
fn copy_reports_failure_if_compositor_has_no_data_control() {
    let fixture = Fixture::with_protocol(None);
    let state = fixture.directory.join("history");
    let id = {
        let mut history = History::open(&state).unwrap();
        history.store("text", b"test").unwrap();
        serde_json::from_slice::<serde_json::Value>(&fs::read(state.join("index.json")).unwrap())
            .unwrap()[0]["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let output = fixture
        .command(&["clipboard", "--state", state.to_str().unwrap(), "copy", &id])
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("ownership was not confirmed")
    );
}

#[test]
fn timed_out_copy_kills_detached_owner_and_cannot_publish_later() {
    let fixture = Fixture::new(true);
    let state = fixture.directory.join("history");
    let id = {
        let mut history = History::open(&state).unwrap();
        history
            .store("text", b"must not appear after failed copy")
            .unwrap();
        serde_json::from_slice::<serde_json::Value>(&fs::read(state.join("index.json")).unwrap())
            .unwrap()[0]["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let (paused, waiting) = mpsc::channel();
    fixture
        .command
        .send(ServerCommand::Pause(Duration::from_secs(6), paused))
        .unwrap();
    waiting.recv_timeout(Duration::from_secs(1)).unwrap();
    let mut copy = fixture
        .command(&["clipboard", "--state", state.to_str().unwrap(), "copy", &id])
        .spawn()
        .unwrap();
    // Only inspect the direct child of this private test's copy command. It is
    // intentionally kept unreaped during handoff and reserves the detached PGID.
    let children = PathBuf::from(format!("/proc/{}/task/{}/children", copy.id(), copy.id()));
    let mut group = None;
    eventually(|| {
        group = fs::read_to_string(&children)
            .ok()
            .and_then(|s| s.split_whitespace().next()?.parse::<i32>().ok());
        group.is_some()
    });
    let group = group.unwrap();
    assert!(!copy.wait().unwrap().success());
    eventually(|| unsafe { libc::kill(-group, 0) < 0 } && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH));
    // A compositor responding late cannot resurrect a failed publisher.
    thread::sleep(Duration::from_millis(1100));
    assert_eq!(fixture.binds.load(Ordering::SeqCst), 0);
}
#[test]
fn watcher_preserves_same_offer_bundle_and_skips_private_or_replayed_selections() {
    let fixture = Fixture::new(true);
    let watcher = fixture.watch();
    eventually(|| fixture.binds.load(Ordering::SeqCst) > 0);
    fixture.replace(
        vec![
            format("text/plain;charset=utf-8", b"first"),
            format("text/html", b"<b>first</b>"),
        ],
        None,
    );
    eventually(|| {
        fixture
            .index()
            .is_some_and(|i| i.as_array().is_some_and(|a| a.len() == 1))
    });
    let index = fixture.index().unwrap();
    let id = index[0]["id"].as_str().unwrap().to_owned();
    let bundle = History::open(&fixture.directory.join("history"))
        .unwrap()
        .bundle(&id)
        .unwrap();
    assert!(
        bundle
            .iter()
            .any(|f| f.mime == "text/html" && f.data == b"<b>first</b>")
    );
    assert!(
        bundle
            .iter()
            .any(|f| f.mime.starts_with("text/plain") && f.data == b"first")
    );
    let received = fixture.receives.load(Ordering::SeqCst);
    fixture.replace(
        vec![
            format("text/plain;charset=utf-8", b"password must not be read"),
            format("x-kde-passwordManagerHint", b"secret"),
        ],
        None,
    );
    thread::sleep(Duration::from_millis(80));
    assert_eq!(fixture.receives.load(Ordering::SeqCst), received);
    fixture.replace(
        vec![
            format("text/plain;charset=utf-8", b"own replay must not be read"),
            format(IDENTITY_MIME, id.as_bytes()),
        ],
        None,
    );
    thread::sleep(Duration::from_millis(80));
    assert_eq!(fixture.receives.load(Ordering::SeqCst), received);
    assert_eq!(fixture.index().unwrap().as_array().unwrap().len(), 1);
    stop(watcher);
}
#[test]
fn new_selection_cancels_unfinished_offer_instead_of_mixing_formats() {
    let fixture = Fixture::new(false);
    let watcher = fixture.watch();
    eventually(|| fixture.binds.load(Ordering::SeqCst) > 0);
    fixture.replace(
        vec![
            format("text/plain;charset=utf-8", b"obsolete"),
            format("text/html", b"<b>obsolete</b>"),
        ],
        Some("text/html"),
    );
    eventually(|| fixture.receives.load(Ordering::SeqCst) == 2);
    assert!(fixture.index().is_none());
    fixture.replace(vec![format("text/plain;charset=utf-8", b"current")], None);
    eventually(|| {
        fixture
            .index()
            .is_some_and(|i| i.as_array().is_some_and(|a| a.len() == 1))
    });
    let index = fixture.index().unwrap();
    assert_eq!(index[0]["preview"], "current");
    let bundle = History::open(&fixture.directory.join("history"))
        .unwrap()
        .bundle(index[0]["id"].as_str().unwrap())
        .unwrap();
    assert_eq!(bundle.len(), 1);
    assert_eq!(bundle[0].data, b"current");
    stop(watcher);
}
