// SPDX-License-Identifier: AGPL-3.0-only
mod protocol;
mod settings;
#[allow(dead_code)]
mod upstream {
    pub mod models;
}
use bluer::{
    Address,
    rfcomm::{Profile, Role, Stream},
};
use futures::StreamExt;
use serde_json::{Value, json};
use std::{
    error::Error,
    io::{self, Write},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    time::{Duration, Instant, timeout, timeout_at},
};
type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
const UUID: &str = "aeac4a03-dff5-498f-843a-34487cf133eb";
fn emit(value: Value) {
    println!("{value}");
    let _ = io::stdout().flush();
}
trait Transport: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Transport for T {}
struct Ear {
    socket: Box<dyn Transport>,
    buffer: Vec<u8>,
    id: u8,
    state: Value,
    battery_at: Option<Instant>,
}
impl Ear {
    fn update(&mut self, packet: &protocol::Packet) {
        if let Some((key, value)) = settings::decode(packet.command, &packet.payload) {
            if key == "battery" {
                self.battery_at = Some(Instant::now());
            }
            self.state[key] = value;
            emit(json!({"event":"state","state":self.state}));
        }
    }
    async fn send(&mut self, command: u16, payload: &[u8]) -> Result<u8> {
        self.id = self.id % 250 + 1;
        timeout(
            Duration::from_secs(3),
            self.socket
                .write_all(&protocol::encode(command, self.id, payload)),
        )
        .await??;
        Ok(self.id)
    }
    async fn next(&mut self) -> Result<protocol::Packet> {
        loop {
            if let Some(packet) = protocol::parse(&mut self.buffer) {
                return Ok(packet);
            }
            let mut bytes = [0; 4096];
            let n = self.socket.read(&mut bytes).await?;
            if n == 0 {
                return Err("Earbuds disconnected".into());
            }
            self.buffer.extend_from_slice(&bytes[..n]);
        }
    }
    async fn query(&mut self, command: u16, responses: &[u16]) -> Result<Vec<u8>> {
        let id = self.send(command, &[]).await?;
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            let packet = timeout_at(end, self.next())
                .await
                .map_err(|_| "Earbuds did not respond within two seconds")??;
            // Some devices deliver battery/ANC as unsolicited E... notifications.
            self.update(&packet);
            if responses.contains(&packet.command)
                && (packet.id == id || packet.command & 0xf000 == 0xe000)
            {
                return Ok(packet.payload);
            }
        }
    }
    async fn change(
        &mut self,
        set: u16,
        payload: &[u8],
        get: u16,
        response: u16,
        key: &str,
        wanted: Value,
    ) -> Result<()> {
        self.send(set, payload).await?;
        let responses = if response == 0x401e {
            vec![0x401e, 0xe003]
        } else {
            vec![response]
        };
        let p = match self.query(get, &responses).await {
            Ok(p) => p,
            Err(error) => {
                self.state[key] = Value::Null;
                emit(json!({"event":"state","state":self.state}));
                return Err(error);
            }
        };
        let (_, actual) = settings::decode(response, &p).ok_or("Invalid setting readback")?;
        if actual != wanted {
            return Err(format!("Device did not confirm {key}; actual state retained").into());
        }
        Ok(())
    }
    async fn action(&mut self, v: Value) -> Result<()> {
        let key = v["setting"].as_str().ok_or("Missing setting")?;
        if self.state[key].is_null() {
            return Err("Setting not available on this device".into());
        }
        match key {
            "anc" => {
                let mode = v["value"]
                    .as_u64()
                    .filter(|m| [1, 2, 3, 4, 5, 7].contains(m))
                    .ok_or("Invalid mode")? as u8;
                self.change(0xf00f, &[1, mode, 0], 0xc01e, 0x401e, key, json!(mode))
                    .await
            }
            "eq" => {
                let mode = v["value"]
                    .as_u64()
                    .filter(|m| [0, 1, 2, 3, 5].contains(m))
                    .ok_or("Invalid EQ")? as u8;
                self.change(0xf010, &[mode, 0], 0xc01f, 0x401f, key, json!(mode))
                    .await
            }
            "listening" => {
                let mode = v["value"]
                    .as_u64()
                    .filter(|m| *m <= 6)
                    .ok_or("Invalid preset")? as u8;
                self.change(0xf01d, &[mode, 0], 0xc050, 0x4050, key, json!(mode))
                    .await
            }
            "inEar" => {
                let value = v["value"].as_bool().ok_or("Invalid toggle")?;
                self.change(
                    0xf004,
                    &[1, 1, u8::from(value)],
                    0xc00e,
                    0x400e,
                    key,
                    json!(value),
                )
                .await
            }
            "latency" => {
                let value = v["value"].as_bool().ok_or("Invalid toggle")?;
                self.change(
                    0xf040,
                    &[if value { 1 } else { 2 }, 0],
                    0xc041,
                    0x4041,
                    key,
                    json!(value),
                )
                .await
            }
            "bass" => {
                let enabled = v["value"]["enabled"].as_bool().ok_or("Invalid toggle")?;
                let level = v["value"]["level"]
                    .as_u64()
                    .filter(|n| (1..=5).contains(n))
                    .ok_or("Invalid bass level")? as u8;
                self.change(
                    0xf051,
                    &[u8::from(enabled), level * 2],
                    0xc04e,
                    0x404e,
                    key,
                    json!({"enabled":enabled,"level":level}),
                )
                .await
            }
            "customEq" => {
                let values = v["value"]
                    .as_array()
                    .filter(|b| b.len() == 3)
                    .ok_or("Three bands required")?;
                let bands: Vec<f32> = values
                    .iter()
                    .map(|n| {
                        n.as_f64()
                            .filter(|n| (-6.0..=6.0).contains(n))
                            .map(|n| n as f32)
                            .ok_or("Invalid band")
                    })
                    .collect::<std::result::Result<_, _>>()?;
                self.change(
                    0xf041,
                    &settings::custom_eq(&bands),
                    0xc044,
                    0x4044,
                    key,
                    json!(bands),
                )
                .await
            }
            "gestures" => {
                let slot = v["slot"].as_u64().ok_or("Missing slot")? as usize;
                let old = self.state[key]
                    .get(slot)
                    .ok_or("Unknown gesture slot")?
                    .clone();
                let action = v["value"]
                    .as_u64()
                    .filter(|a| [1, 8, 9, 10, 11, 18, 19, 20, 21, 22].contains(a))
                    .ok_or("Unsupported gesture action")? as u8;
                let kind = old["kind"].as_u64().ok_or("Invalid gesture type")?;
                let safe = match kind {
                    2 | 3 => [8, 9, 11].contains(&action),
                    7 => [10, 11, 18, 19, 20, 21, 22].contains(&action),
                    9 => [1, 10, 11, 18, 19, 20, 21, 22].contains(&action),
                    _ => false,
                };
                if !safe || ![2, 3].contains(&old["device"].as_u64().unwrap_or(0)) {
                    return Err("Unsupported gesture configuration".into());
                }
                let mut wanted = self.state[key].clone();
                wanted[slot]["action"] = json!(action);
                self.change(
                    0xf003,
                    &[
                        1,
                        old["device"].as_u64().unwrap() as u8,
                        old["common"].as_u64().unwrap() as u8,
                        old["kind"].as_u64().unwrap() as u8,
                        action,
                    ],
                    0xc018,
                    0x4018,
                    key,
                    wanted,
                )
                .await
            }
            _ => Err("Unknown setting".into()),
        }
    }
}
async fn run() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .ok_or("Usage: foundation-nothing /org/bluez/hciN/dev_XX...")?;
    if path == "--discover" {
        let session = bluer::Session::new().await?;
        let uuid = uuid::Uuid::parse_str(UUID)?;
        let mut paths = vec![];
        for name in session.adapter_names().await? {
            let adapter = session.adapter(&name)?;
            for address in adapter.device_addresses().await? {
                let device = adapter.device(address)?;
                if device.is_paired().await?
                    && device.uuids().await?.unwrap_or_default().contains(&uuid)
                {
                    paths.push(format!(
                        "/org/bluez/{name}/dev_{}",
                        address.to_string().replace(':', "_")
                    ));
                }
            }
        }
        emit(json!({"devices":paths}));
        return Ok(());
    }
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 5
        || parts[..3] != ["", "org", "bluez"]
        || !parts[3].starts_with("hci")
        || !parts[4].starts_with("dev_")
    {
        return Err("Invalid BlueZ device path".into());
    }
    let address: Address = parts[4][4..].replace('_', ":").parse()?;
    let session = bluer::Session::new().await?;
    let adapter = session.adapter(parts[3])?;
    let device = adapter.device(address)?;
    if !device.is_paired().await? || !device.is_connected().await? {
        return Err("Connect the paired earbuds first".into());
    }
    let uuid = uuid::Uuid::parse_str(UUID)?;
    if !device.uuids().await?.unwrap_or_default().contains(&uuid) {
        return Err("Device does not advertise Nothing controls".into());
    }
    let mut profile = session
        .register_profile(Profile {
            uuid,
            name: Some("Desktop Foundation earbud controls".into()),
            role: Some(Role::Client),
            require_authentication: Some(true),
            auto_connect: Some(false),
            ..Default::default()
        })
        .await?;
    let connect = device.connect_profile(&uuid);
    let accept = async {
        while let Some(request) = profile.next().await {
            if request.device() == address {
                return Ok(request.accept()?);
            }
            request.reject(bluer::rfcomm::ReqError::Rejected);
        }
        Err::<Stream, Box<dyn Error + Send + Sync>>("BlueZ profile closed".into())
    };
    let (_, socket) = timeout(Duration::from_secs(10), async {
        tokio::try_join!(
            async {
                connect
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)
            },
            accept
        )
    })
    .await??;
    let mut ear = Ear {
        socket: Box::new(socket),
        buffer: vec![],
        id: 0,
        state: json!({}),
        battery_at: None,
    };
    let serial = ear.query(0xc006, &[0x4006]).await?;
    let model = settings::identify(&serial)
        .ok_or("Unrecognized Nothing/CMF model; generic Bluetooth remains available")?;
    ear.state["model"] = json!(model.name);
    ear.state["modelCode"] = json!(model.base.as_str());
    emit(json!({"event":"identified","state":ear.state}));
    let mut requests = vec![
        (0xc007, 0x4007),
        (0xc042, 0x4042),
        (0xc041, 0x4041),
        (0xc018, 0x4018),
    ];
    if model.anc_capable {
        requests.push((0xc01e, 0x401e));
    }
    requests.push(if model.base.supports_listening_modes() {
        (0xc050, 0x4050)
    } else {
        (0xc01f, 0x401f)
    });
    if model.base.supports_in_ear_detection() {
        requests.push((0xc00e, 0x400e));
    }
    if model.base.supports_enhanced_bass() {
        requests.push((0xc04e, 0x404e));
    }
    if model.base.supports_custom_eq() {
        requests.push((0xc044, 0x4044));
    }
    // The references only expose advanced-EQ enabled state, not a reliable band codec.
    requests.push((0xc04c, 0x404c));
    for (get, response) in requests {
        let responses = match get {
            0xc007 => vec![0x4007, 0xe001],
            0xc01e => vec![0x401e, 0xe003],
            _ => vec![response],
        };
        if let Err(e) = ear.query(get, &responses).await {
            emit(json!({"event":"unavailable","command":get,"reason":e.to_string()}));
        }
    }
    emit(json!({"event":"ready","state":ear.state}));
    ear.state["findAvailable"] = json!(!model.base.is_over_ear());
    emit(json!({"event":"state","state":ear.state}));
    let mut ringing: Option<(u8, Instant)> = None;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut events = device.events().await?;
    let outcome: Result<()> = async {
    loop {
        let stale = ear
            .battery_at
            .map(|t| t + Duration::from_secs(120))
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(86400));
        let ring_end = ringing
            .map(|(_, end)| end)
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(86400));
        tokio::select! {
            _=terminate.recv()=>break,
            _=tokio::time::sleep_until(ring_end), if ringing.is_some()=>{
                let (side,_)=ringing.take().unwrap();
                let payload=if model.base.as_str()=="B181" {vec![0]} else {vec![side,0]};
                ear.send(0xf002,&payload).await?;
                emit(json!({"event":"complete","success":true}));
            },
            line=lines.next_line()=>{
                let Some(line)=line? else {break;};
                if line.len()>8192 {continue;}
                let value:Value=match serde_json::from_str(&line) {Ok(v)=>v,Err(_)=>{emit(json!({"event":"error","message":"Invalid command"}));continue;}};
                if value["action"]=="close" {break;}
                if value["action"]=="ring" {
                    let side=value["side"].as_u64().filter(|s|[2,3].contains(s)).ok_or("Invalid earbud")? as u8;
                    if !model.base.is_over_ear() && ringing.is_none() {
                        let payload=if model.base.as_str()=="B181" {vec![1]} else {vec![side,1]};
                        ear.send(0xf002,&payload).await?;
                        ringing=Some((side,Instant::now()+Duration::from_secs(3)));
                        emit(json!({"event":"ringing","side":side}));
                    }
                } else if value["action"]=="refresh" {
                    // User-requested refresh, not a polling timer.
                    let result=ear.query(0xc007,&[0x4007,0xe001]).await;
                    match result {
                        Ok(_)=>emit(json!({"event":"complete","success":true})),
                        Err(e)=>{
                            ear.state["battery"]=Value::Null;ear.battery_at=None;
                            emit(json!({"event":"state","state":ear.state}));
                            emit(json!({"event":"error","message":e.to_string()}));
                        }
                    }
                } else {
                    let result=ear.action(value).await;
                    emit(match result {Ok(())=>json!({"event":"complete","success":true}),Err(e)=>json!({"event":"error","message":e.to_string()})});
                }
            },
            packet=ear.next()=>ear.update(&packet?),
            event=events.next()=>{
                match event {Some(bluer::DeviceEvent::PropertyChanged(bluer::DeviceProperty::Connected(false)))|None=>return Err("Bluetooth connection closed; reopen Controls after reconnecting".into()),_=>{}}
            },
            _=tokio::time::sleep_until(stale), if ear.battery_at.is_some()=>{
                ear.state["battery"]=Value::Null; ear.battery_at=None;
                emit(json!({"event":"state","state":ear.state}));
            }
        }
    }
    Ok(())
    }.await;
    if let Some((side, _)) = ringing {
        let payload = if model.base.as_str() == "B181" {
            vec![0]
        } else {
            vec![side, 0]
        };
        let _ = ear.send(0xf002, &payload).await;
    }
    drop(profile);
    outcome
}
#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        emit(json!({"event":"error","fatal":true,"message":error.to_string()}));
        std::process::exit(1);
    }
    // Tokio stdin uses a blocking reader; do not wait for it at runtime shutdown.
    // run() has already closed the RFCOMM socket and dropped the profile handle.
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Ear, tokio::io::DuplexStream) {
        let (client, server) = tokio::io::duplex(8192);
        (
            Ear {
                socket: Box::new(client),
                buffer: vec![],
                id: 0,
                state: json!({}),
                battery_at: None,
            },
            server,
        )
    }
    #[tokio::test]
    async fn correlation_and_unsolicited_battery() {
        let (mut ear, mut server) = fixture();
        tokio::spawn(async move {
            let mut request = [0; 10];
            server.read_exact(&mut request).await.unwrap();
            server
                .write_all(&protocol::encode(0x401f, 99, &[3]))
                .await
                .unwrap();
            server
                .write_all(&protocol::encode(0xe001, 0, &[1, 2, 85]))
                .await
                .unwrap();
            server
                .write_all(&protocol::encode(0x401f, request[7], &[0]))
                .await
                .unwrap();
        });
        assert_eq!(ear.query(0xc01f, &[0x401f]).await.unwrap(), vec![0]);
        assert_eq!(ear.state["battery"]["left"]["percent"], 85);
        assert!(ear.state["battery"]["right"].is_null());
    }
    #[tokio::test]
    async fn events_cannot_extend_deadline() {
        let (mut ear, mut server) = fixture();
        let task = tokio::spawn(async move {
            let mut request = [0; 10];
            server.read_exact(&mut request).await.unwrap();
            loop {
                server
                    .write_all(&protocol::encode(0xe001, 0, &[1, 2, 85]))
                    .await
                    .unwrap();
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });
        let start = Instant::now();
        assert!(ear.query(0xc01f, &[0x401f]).await.is_err());
        assert!(start.elapsed() < Duration::from_secs(3));
        task.abort();
    }
    #[tokio::test]
    async fn changed_setting_requires_actual_readback() {
        let (mut ear, mut server) = fixture();
        tokio::spawn(async move {
            let mut set = [0; 12];
            server.read_exact(&mut set).await.unwrap();
            let mut request = [0; 10];
            server.read_exact(&mut request).await.unwrap();
            server
                .write_all(&protocol::encode(0x401f, request[7], &[0]))
                .await
                .unwrap();
        });
        assert!(
            ear.change(0xf010, &[3, 0], 0xc01f, 0x401f, "eq", json!(3))
                .await
                .is_err()
        );
        assert_eq!(ear.state["eq"], 0);
    }
}
