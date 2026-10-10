//! Line-oriented integration interface around the actual native client core.
use anyhow::{Context as _, Result};
use clap::Parser;
use cords_client_core::client::{Client, Notification};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use tokio::io::{AsyncBufReadExt as _, BufReader};
use zeroize::Zeroizing;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long)]
    state: PathBuf,
    #[arg(long, default_value = "migrations/sqlite")]
    migrations: PathBuf,
    #[arg(long)]
    ca: Option<PathBuf>,
    /// Read the unlock passphrase as the first stdin line, never a process argument.
    #[arg(long)]
    passphrase_stdin: bool,
}
fn emit(value: &Value) {
    println!("{value}");
}
fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v.get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("missing {key}"))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    let passphrase = if args.passphrase_stdin {
        Some(Zeroizing::new(
            input.next_line().await?.context("missing passphrase")?,
        ))
    } else {
        None
    };
    let mut client = Client::open(
        &args.state,
        &args.migrations,
        passphrase.as_ref().map(|v| v.as_bytes()),
        args.ca.as_deref(),
    )
    .await?;
    drop(passphrase);
    emit(
        &json!({"event":"ready","status":client.status(),"fault_injection":cfg!(feature="fault-injection")}),
    );
    let mut task: Option<tokio::task::JoinHandle<()>> = None;
    let mut receiver: Option<tokio::sync::mpsc::Receiver<Notification>> = None;
    let mut wanted = false;
    let mut timer = tokio::time::interval(Duration::from_secs(3));
    loop {
        tokio::select! {
            line=input.next_line()=>{
                let Some(line)=line? else{break;};
                let request:Value=serde_json::from_str(&line)?;let request_id=request.get("id").cloned().unwrap_or(Value::Null);
                let operation=field(&request,"op")?;
                if operation=="exit" {emit(&json!({"id":request_id,"ok":true}));break;}
                let result:Result<Value>=async {
                    Ok(match operation {
                        "status"=>serde_json::to_value(client.status())?,
                        "trust"=>serde_json::to_value(client.trust(field(&request,"origin")?).await?)?,
                        "claim-ownership"=>serde_json::to_value(client.claim_ownership(field(&request,"code")?).await?)?,
                        "authenticate"=>serde_json::to_value(client.authenticate().await?)?,
                        "publish"=>client.publish_key_package().await?,
                        "create"=>json!(client.create_channel(field(&request,"name")?).await?),
                        "members"=>serde_json::to_value(client.members().await?)?,
                        "channels"=>serde_json::to_value(client.channels().await?)?,
                        "add"=>{client.add_member(field(&request,"route")?,field(&request,"device")?).await?;json!(true)},
                        "remove"=>{client.remove_member(field(&request,"route")?,field(&request,"device")?).await?;json!(true)},
                        "pending-removals"=>serde_json::to_value(client.pending_removals(field(&request,"route")?).await?)?,
                        "revoke"=>{client.revoke_device().await?;json!(true)},
                        "join"=>{client.join_channel(field(&request,"route")?).await?;json!(true)},
                        "send"=>json!(client.send(field(&request,"route")?,field(&request,"body")?).await?),
                        "retry"=>serde_json::to_value(client.retry_last().await?)?,
                        "sync"=>serde_json::to_value(client.synchronize().await?)?,
                        "history"=>serde_json::to_value(client.history(field(&request,"route")?).await?)?,
                        "connect"=>{
                            if let Some(old)=task.take(){old.abort();}
                            let (handle,receive)=client.notifications().await?;task=Some(handle);receiver=Some(receive);wanted=true;
                            serde_json::to_value(client.synchronize().await?)?
                        }
                        "disconnect"=>{wanted=false;if let Some(old)=task.take(){old.abort();}receiver=None;json!(true)},
                        _=>anyhow::bail!("unknown operation"),
                    })
                }.await;
                match result {
                    Ok(value)=>emit(&json!({"id":request_id,"ok":true,"result":value})),
                    Err(error)=>{client.reload().await?;emit(&json!({"id":request_id,"ok":false,"error":error.to_string()}));}
                }
            }
            notice=async {match &mut receiver {Some(receiver)=>receiver.recv().await,None=>std::future::pending().await}}=>{
                if let Some(Notification::Advanced) = notice { match client.synchronize().await {
                    Ok(result)=>emit(&json!({"event":"realtime","result":result})),
                    Err(error)=>{client.reload().await?;emit(&json!({"event":"error","error":error.to_string()}));}
                } } else {receiver=None;task=None;emit(&json!({"event":"disconnected"}));}
            }
            _=timer.tick(),if wanted=>{
                if receiver.is_none(){
                    match client.notifications().await {
                        Ok((handle,receive))=>{task=Some(handle);receiver=Some(receive);emit(&json!({"event":"reconnected"}));},
                        Err(error)=>{emit(&json!({"event":"connection_error","error":error.to_string()}));continue;}
                    }
                }
                match client.synchronize().await {
                    Ok(result)=>{if result.fetched>0{emit(&json!({"event":"catchup","result":result}));}},
                    Err(error)=>{client.reload().await?;emit(&json!({"event":"sync_error","error":error.to_string()}));}
                }
            }
        }
    }
    if let Some(task) = task {
        task.abort();
    }
    Ok(())
}
