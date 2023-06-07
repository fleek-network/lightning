#![feature(core_intrinsics)]

use std::time::Duration;

use criterion::{measurement::Measurement, *};
use draco_handshake::connection::{HandshakeConnection, HandshakeFrame, Reason};
use draco_interfaces::CompressionAlgoSet;
use fleek_crypto::{ClientPublicKey, ClientSignature, NodePublicKey};
use futures::executor::block_on;
use tokio::sync::Mutex;

mod transport {
    use bytes::BytesMut;
    use tokio::io::{AsyncRead, AsyncWrite};

    #[derive(Clone, Default)]
    /// DummyReader always returns a clone of the internal buffer
    pub struct DummyReader(pub BytesMut);

    impl AsyncRead for DummyReader {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            // always return the same frame
            let bytes = self.0.clone();
            buf.put_slice(&bytes);
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[derive(Clone, Default)]
    /// DummyWriter holds the last thing written to it in a buffer
    pub struct DummyWriter(pub BytesMut);

    impl AsyncWrite for DummyWriter {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<Result<usize, std::io::Error>> {
            // store the last thing written to out_buf
            let out_buf = &mut self.get_mut().0;
            *out_buf = BytesMut::from(buf);
            std::task::Poll::Ready(Ok(out_buf.len()))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Result<(), std::io::Error>> {
            std::task::Poll::Ready(Ok(()))
        }
    }
}

fn bench_frame<T: Measurement>(
    g: &mut BenchmarkGroup<T>,
    frame: HandshakeFrame,
    title: &'static str,
) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    g.throughput(Throughput::Bytes(frame.size_hint() as u64));

    let reader = transport::DummyReader::default();
    let writer = transport::DummyWriter::default();

    g.bench_function(format!("{title}/write"), |b| {
        let conn = Mutex::new(HandshakeConnection::new(reader.clone(), writer.clone()));
        b.to_async(&runtime).iter(|| async {
            // executed sequentially so should be a minimal await
            let mut conn = conn.lock().await;
            conn.write_frame(frame.clone()).await
        })
    });

    // setup for decoding
    let (reader, writer) = block_on(async move {
        let mut conn = HandshakeConnection::new(reader, writer);
        conn.write_frame(frame.clone()).await.unwrap();
        // writer contains the encoded frame, so we put that into the reader to be decoded
        conn.reader.0 = conn.writer.0.split();
        conn.finish()
    });

    g.bench_function(format!("{title}/read"), |b| {
        let conn = Mutex::new(HandshakeConnection::new(reader.clone(), writer.clone()));
        b.to_async(&runtime).iter(|| async {
            let mut conn = conn.lock().await;
            conn.read_frame(None).await
        })
    });
}

fn bench_codec_group(c: &mut Criterion) {
    let mut g = c.benchmark_group("Handshake Connection Frames");
    g.sample_size(20);
    g.measurement_time(Duration::from_secs(15));

    let frame = HandshakeFrame::HandshakeRequest {
        version: 0,
        supported_compression_set: CompressionAlgoSet::new(),
        pubkey: ClientPublicKey([0u8; 20]),
        resume_lane: None,
    };
    bench_frame(&mut g, frame, "handshake_request");

    let frame = HandshakeFrame::HandshakeResponse {
        pubkey: NodePublicKey([3u8; 96]),
        nonce: 65535,
        lane: 0,
    };
    bench_frame(&mut g, frame, "handshake_response");

    let frame = HandshakeFrame::HandshakeResponseUnlock {
        pubkey: NodePublicKey([3u8; 96]),
        nonce: 65535,
        lane: 0,
        last_bytes: 420,
        last_service_id: 0,
        last_signature: [3u8; 96],
    };
    bench_frame(&mut g, frame, "handshake_response_unlock");

    let frame = HandshakeFrame::ServiceRequest { service_id: 0 };
    bench_frame(&mut g, frame, "service_request");

    let frame = HandshakeFrame::DeliveryAcknowledgement {
        // TODO: get size for client signature in fleek_crypto
        signature: ClientSignature,
    };
    bench_frame(&mut g, frame, "delivery_acknowledgement");

    let frame = HandshakeFrame::TerminationSignal(Reason::Unknown);
    bench_frame(&mut g, frame, "termination_signal");

    g.finish();
}

criterion_group!(benches, bench_codec_group);
criterion_main!(benches);
