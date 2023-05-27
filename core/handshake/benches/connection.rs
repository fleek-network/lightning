#![feature(core_intrinsics)]

use std::time::Duration;

use criterion::{measurement::Measurement, *};
use draco_handshake::connection::{HandshakeConnection, HandshakeFrame, Reason};
use futures::executor::block_on;
use tokio::sync::Mutex;

mod transport {
    use bytes::BytesMut;
    use tokio::io::{AsyncRead, AsyncWrite};

    /// Direct transport for benchmarking frames.
    /// - `in_buf`: Holds a frame to read, every time.
    /// - `out_buf`: Holds the last frame written.
    #[derive(Default, Clone)]
    pub struct DirectTransport {
        pub in_buf: BytesMut,
        pub out_buf: BytesMut,
    }

    impl AsyncRead for DirectTransport {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            // always return the same frame
            let bytes = self.in_buf.clone();
            buf.put_slice(&bytes);
            std::task::Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for DirectTransport {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> std::task::Poll<Result<usize, std::io::Error>> {
            // store the last thing written to out_buf
            let out_buf = &mut self.get_mut().out_buf;
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
    let transport = transport::DirectTransport::default();

    g.bench_function(format!("{title}/write"), |b| {
        let conn = Mutex::new(HandshakeConnection::new(transport.clone()));
        b.to_async(&runtime).iter(|| async {
            // executed sequentially so should be a minimal await
            let mut conn = conn.lock().await;
            conn.write_frame(frame.clone()).await
        })
    });

    // setup for decoding
    let transport = block_on(async move {
        let mut conn = HandshakeConnection::new(transport);
        conn.write_frame(frame.clone()).await.unwrap();
        // out_buf contains the encoded frame, so we take that into in_buf to be decoded
        conn.stream.in_buf = conn.stream.out_buf.split();
        conn.stream
    });

    g.bench_function(format!("{title}/read"), |b| {
        let conn = Mutex::new(HandshakeConnection::new(transport.clone()));
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
        supported_compression_bitmap: 0,
        pubkey: [3u8; 48],
        resume_lane: None,
    };
    bench_frame(&mut g, frame, "handshake_request");

    let frame = HandshakeFrame::HandshakeResponse {
        pubkey: [3u8; 33],
        nonce: 65535,
        lane: 0,
    };
    bench_frame(&mut g, frame, "handshake_response");

    let frame = HandshakeFrame::HandshakeResponseUnlock {
        pubkey: [3u8; 33],
        nonce: 65535,
        lane: 0,
        last_bytes: 420,
        last_service_id: [0u8; 32],
        last_signature: [3u8; 96],
    };
    bench_frame(&mut g, frame, "handshake_response_unlock");

    let frame = HandshakeFrame::ServiceRequest {
        service_id: [3u8; 32],
    };
    bench_frame(&mut g, frame, "service_request");

    let frame = HandshakeFrame::DeliveryAcknowledgement {
        signature: [3u8; 96],
    };
    bench_frame(&mut g, frame, "delivery_acknowledgement");

    let frame = HandshakeFrame::TerminationSignal(Reason::Unknown);
    bench_frame(&mut g, frame, "termination_signal");

    g.finish();
}

criterion_group!(benches, bench_codec_group);
criterion_main!(benches);
