use lightning_openrpc::Project;
use lightning_rpc::api::{FleekApiOpenRpc, NetApiOpenRpc};
use std::fs::File;
use clap::Parser;

const VERSION: &str = "1.0.0";
const TITLE: &str = "Fleek Lightning";
const DESCRIPTION: &str = "Fleek Lightning RPC API";
const CONTACT_NAME: &str = "";
const URL: &str = "";
const EMAIL: &str = "";
const LICENSE: &str = "";
const LICENSE_URL: &str = "";

#[derive(Debug, Clone, Parser)]
#[command(about, version)]
struct Args {
    /// If present the path to write the generated spec to
    /// 
    /// '/path/to/spec/'
    #[arg(short, long)]
    write_to: Option<std::path::PathBuf>
}

fn main(){
    let Args {
        write_to
    } = Args::parse();

    let mut project = Project::new(
        VERSION,
        TITLE,
        DESCRIPTION,
        CONTACT_NAME,
        URL,
        EMAIL,
        LICENSE,
        LICENSE_URL,
    );

    project.add_module(FleekApiOpenRpc::module_doc());
    project.add_module(NetApiOpenRpc::module_doc());

    if let Some(path) = write_to {
        let path = path.join("openrpc.json");
        let file = File::create(&path).unwrap();
        serde_json::to_writer_pretty(file, &project).unwrap();
        println!("Printed to {:?}", path);
    } else {
        let content = serde_json::to_string_pretty(&project).unwrap();
        println!("{content}");
    }
}
