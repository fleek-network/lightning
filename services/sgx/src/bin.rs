use std::future::Future;
use std::io::Result as IoResult;
use std::pin::Pin;
use std::process::Stdio;
use std::task::{Context, Poll};

use aesm_client::AesmClient;
use enclave_runner::usercalls::{AsyncStream, UsercallExtension};
use enclave_runner::EnclaveBuilder;
use futures::FutureExt;
use sgxs_loaders::isgx::Device as IsgxDevice;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::{ChildStdin, ChildStdout, Command};

const ENCLAVE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/enclave.sgxs"));

/// This example demonstrates use of usercall extensions.
/// User call extension allow the enclave code to "connect" to an external service via a customized
/// enclave runner. Here we customize the runner to intercept calls to connect to an address "cat"
/// which actually connects the enclave application to stdin and stdout of `cat` process.
struct CatService {
    stdin: ChildStdin,
    stdout: ChildStdout,
}

impl CatService {
    // SAFETY: `Self` doesn't implement `Drop` or `Unpin`, and isn't `repr(packed)`
    pin_utils::unsafe_pinned!(stdin: ChildStdin);
    pin_utils::unsafe_pinned!(stdout: ChildStdout);

    fn new() -> Result<CatService, std::io::Error> {
        Command::new("/bin/cat")
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .map(|mut c| CatService {
                stdin: c.stdin.take().unwrap(),
                stdout: c.stdout.take().unwrap(),
            })
    }
}

impl AsyncRead for CatService {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context, buf: &mut [u8]) -> Poll<IoResult<usize>> {
        self.stdout().poll_read(cx, buf)
    }
}

impl AsyncWrite for CatService {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context, buf: &[u8]) -> Poll<IoResult<usize>> {
        self.stdin().poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context) -> Poll<IoResult<()>> {
        self.stdin().poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context) -> Poll<IoResult<()>> {
        self.stdin().poll_shutdown(cx)
    }
}

#[derive(Debug)]
struct ExternalService;

impl UsercallExtension for ExternalService {
    fn connect_stream<'future>(
        &'future self,
        addr: &'future str,
        _local_addr: Option<&'future mut String>,
        _peer_addr: Option<&'future mut String>,
    ) -> std::pin::Pin<Box<dyn Future<Output = IoResult<Option<Box<dyn AsyncStream>>>> + 'future>>
    {
        async move {
            // If the passed address is not "service", we return none, whereby the passed address
            // gets treated as an IP address which is the default behavior.
            match addr {
                "service" => {
                    let stream = CatService::new()?;
                    Ok(Some(Box::new(stream) as _))
                },
                _ => Ok(None),
            }
        }
        .boxed_local()
    }
}

fn main() {
    let mut device = IsgxDevice::new()
        .unwrap()
        .einittoken_provider(AesmClient::new())
        .build();

    let mut enclave_builder = EnclaveBuilder::new_from_memory(ENCLAVE);
    enclave_builder.dummy_signature();
    enclave_builder.usercall_extension(ExternalService);
    let enclave = enclave_builder.build(&mut device).unwrap();

    enclave
        .run()
        .map_err(|e| {
            println!("Error while executing SGX enclave.\n{}", e);
            std::process::exit(1)
        })
        .expect("Error while executing SGX enclave.");
}
