use std::sync::Arc;

use anyhow::{anyhow, Result};
use smoltcp::wire::IpRepr;
use smoltcp::{
    phy::ChecksumCapabilities,
    wire::{
        IpAddress, IpProtocol, Ipv4Address, Ipv4Packet, Ipv4Repr, TcpControl, TcpPacket, TcpRepr,
        TcpSeqNumber, UdpPacket, UdpRepr,
    },
};
use tokio::sync::{
    mpsc::{channel, unbounded_channel, Receiver, Sender, UnboundedSender},
    Notify,
};
use tokio::task::JoinHandle;

use crate::messages::{IpPacket, NetworkCommand, NetworkEvent, TransportCommand, TransportEvent};

use super::task::NetworkTask;

struct MockNetwork {
    wg_to_smol_tx: Sender<NetworkEvent>,
    smol_to_wg_rx: Receiver<NetworkCommand>,

    py_to_smol_tx: UnboundedSender<TransportCommand>,
    smol_to_py_rx: Receiver<TransportEvent>,

    sd_trigger: Arc<Notify>,
    handle: JoinHandle<Result<()>>,
}

impl MockNetwork {
    pub fn init() -> Result<Self> {
        let (wg_to_smol_tx, wg_to_smol_rx) = channel(16);
        let (smol_to_wg_tx, smol_to_wg_rx) = channel(16);

        let (py_to_smol_tx, py_to_smol_rx) = unbounded_channel();
        let (smol_to_py_tx, smol_to_py_rx) = channel(64);

        let sd_trigger = Arc::new(Notify::new());

        let task = NetworkTask::new(
            smol_to_wg_tx,
            wg_to_smol_rx,
            smol_to_py_tx,
            py_to_smol_rx,
            sd_trigger.clone(),
        )?;

        let handle = tokio::spawn(task.run());

        Ok(Self {
            wg_to_smol_tx,
            smol_to_wg_rx,
            py_to_smol_tx,
            smol_to_py_rx,
            sd_trigger,
            handle,
        })
    }

    async fn stop(self) -> Result<()> {
        self.sd_trigger.notify_waiters();
        self.handle.await?
    }

    async fn push_wg_packet(&self, packet: IpPacket) -> Result<()> {
        let event = NetworkEvent::ReceivePacket(packet);
        Ok(self.wg_to_smol_tx.send(event).await?)
    }

    #[allow(unused)]
    fn push_py_command(&self, command: TransportCommand) -> Result<()> {
        Ok(self.py_to_smol_tx.send(command)?)
    }

    #[allow(unused)]
    async fn pull_wg_packet(&mut self) -> Option<IpPacket> {
        self.smol_to_wg_rx.recv().await.map(|command| {
            let NetworkCommand::SendPacket(packet) = command;
            packet
        })
    }

    async fn pull_py_event(&mut self) -> Option<TransportEvent> {
        self.smol_to_py_rx.recv().await
    }
}

#[allow(clippy::too_many_arguments)]
fn build_ipv4_tcp_packet(
    src_addr: Ipv4Address,
    dst_addr: Ipv4Address,
    src_port: u16,
    dst_port: u16,
    control: TcpControl,
    seq_number: TcpSeqNumber,
    ack_number: Option<TcpSeqNumber>,
    payload: &[u8],
) -> Ipv4Packet<Vec<u8>> {
    let tcp_repr = TcpRepr {
        src_port,
        dst_port,
        control,
        seq_number,
        ack_number,
        window_len: 64240,
        window_scale: Some(8),
        max_seg_size: Some(1380),
        sack_permitted: true,
        sack_ranges: [None, None, None],
        payload,
    };

    let ip_repr = Ipv4Repr {
        src_addr,
        dst_addr,
        protocol: IpProtocol::Tcp,
        payload_len: tcp_repr.header_len() + payload.len(),
        hop_limit: 255,
    };

    let buf = vec![0u8; IpRepr::Ipv4(ip_repr).total_len()];

    let mut ip_packet = Ipv4Packet::new_unchecked(buf);
    ip_repr.emit(&mut ip_packet, &ChecksumCapabilities::default());

    tcp_repr.emit(
        &mut TcpPacket::new_unchecked(ip_packet.payload_mut()),
        &ip_repr.src_addr.into(),
        &ip_repr.dst_addr.into(),
        &ChecksumCapabilities::default(),
    );

    ip_packet
}

fn build_ipv4_udp_packet(
    src_addr: Ipv4Address,
    dst_addr: Ipv4Address,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Ipv4Packet<Vec<u8>> {
    let udp_repr = UdpRepr { src_port, dst_port };

    let ip_repr = Ipv4Repr {
        src_addr,
        dst_addr,
        protocol: IpProtocol::Udp,
        payload_len: udp_repr.header_len() + payload.len(),
        hop_limit: 255,
    };

    let buf = vec![0u8; IpRepr::Ipv4(ip_repr).total_len()];

    let mut ip_packet = Ipv4Packet::new_unchecked(buf);
    ip_repr.emit(&mut ip_packet, &ChecksumCapabilities::default());

    udp_repr.emit(
        &mut UdpPacket::new_unchecked(ip_packet.payload_mut()),
        &ip_repr.src_addr.into(),
        &ip_repr.dst_addr.into(),
        payload.len(),
        |buf| buf.copy_from_slice(payload),
        &ChecksumCapabilities::default(),
    );

    ip_packet
}

#[tokio::test]
async fn do_nothing() -> Result<()> {
    let mock = MockNetwork::init()?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    mock.stop().await
}

#[tokio::test]
async fn receive_datagram() -> Result<()> {
    let mut mock = MockNetwork::init()?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let src_addr = Ipv4Address([10, 0, 0, 1]);
    let dst_addr = Ipv4Address([10, 0, 0, 42]);
    let data = "hello world!".as_bytes();

    let udp_ip_packet = build_ipv4_udp_packet(src_addr, dst_addr, 1234, 31337, data);

    mock.push_wg_packet(udp_ip_packet.into()).await?;
    let event = mock.pull_py_event().await.unwrap();

    if let TransportEvent::DatagramReceived {
        data: recv_data,
        src_addr: recv_src_addr,
        dst_addr: recv_dst_addr,
    } = event
    {
        assert_eq!(data, recv_data);
        assert_eq!(IpAddress::Ipv4(src_addr), recv_src_addr.ip().into());
        assert_eq!(IpAddress::Ipv4(dst_addr), recv_dst_addr.ip().into());
    } else {
        return Err(anyhow!("Wrong Transport event emitted!"));
    }

    mock.stop().await
}

#[tokio::test]
async fn connection_established() -> Result<()> {
    let mut mock = MockNetwork::init()?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let mut seq = TcpSeqNumber(rand::random::<i32>());

    let src_addr = Ipv4Address([10, 0, 0, 1]);
    let dst_addr = Ipv4Address([10, 0, 0, 42]);
    let data = "hello world!".as_bytes();

    // send TCP SYN
    let tcp_ip_syn_packet = build_ipv4_tcp_packet(
        src_addr,
        dst_addr,
        1234,
        31337,
        TcpControl::Syn,
        seq,
        None,
        &[],
    );
    mock.push_wg_packet(tcp_ip_syn_packet.into()).await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // expect TCP SYN/ACK
    let mut tcp_synack_ip_packet = match mock.pull_wg_packet().await.unwrap() {
        IpPacket::V4(packet) => packet,
        IpPacket::V6(_) => return Err(anyhow!("Received unexpected IPv6 packet!")),
    };

    let synack_src_addr = tcp_synack_ip_packet.src_addr();
    let synack_dst_addr = tcp_synack_ip_packet.dst_addr();

    let tcp_synack_repr = TcpRepr::parse(
        &TcpPacket::new_unchecked(tcp_synack_ip_packet.payload_mut()),
        &synack_src_addr.into(),
        &synack_dst_addr.into(),
        &ChecksumCapabilities::default(),
    )
    .unwrap();

    assert_eq!(tcp_synack_repr.control, TcpControl::Syn);
    assert_eq!(tcp_synack_repr.ack_number.unwrap(), seq + 1);
    let ack = tcp_synack_repr.seq_number + 1;

    // send TCP ACK
    seq += 1;

    let tcp_ip_ack_packet = build_ipv4_tcp_packet(
        src_addr,
        dst_addr,
        1234,
        31337,
        TcpControl::None,
        seq,
        Some(ack),
        data,
    );
    mock.push_wg_packet(tcp_ip_ack_packet.into()).await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // expect ConnectionEstablished event
    let event = mock.pull_py_event().await.unwrap();

    if let TransportEvent::ConnectionEstablished {
        connection_id: _,
        src_addr: recv_src_addr,
        dst_addr: recv_dst_addr,
    } = event
    {
        assert_eq!(IpAddress::Ipv4(src_addr), recv_src_addr.ip().into());
        assert_eq!(IpAddress::Ipv4(dst_addr), recv_dst_addr.ip().into());
    } else {
        return Err(anyhow!("Wrong Transport event emitted!"));
    }

    mock.stop().await
}