use rive::prelude::*;
use std::{env, error::Error};
use regex::Regex;

const INVALID_CHANNELS:[&str; 3] = [
	"01HBPSCHW964KDCGF7RC4HCCSR", // Barry, 63: test2 (deny)
	"01FD53QCD84PX7D2GBV5SBE09N", // Revolt: Submit to Discover
	"01HC0P7QBKYPHH97D1ZMD9E9BC", // Revolt: Looking for Group
];

const OWNER_ID:&str = "01GV7GN0H4JT7EWG5GY64RA2VV";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	// version with HTTPS in front of it, but that isnt needed for the links
	//let inviteRipper:Regex = Regex::new(r"(?<link>https:\/\/rvlt.gg\/[\w|\d]{8})").unwrap();
	let linkRipper:Regex = Regex::new(r"(?<link>rvlt.gg\/[\w|\d]{8})").unwrap();
	let linkIdRipper:Regex = Regex::new(r"rvlt.gg\/(?<link>[\w|\d]{8})").unwrap();
	let idRipper:Regex = Regex::new(r"(?<link>[\dA-Z]{26})").unwrap();
	let userAuth:Authentication = Authentication::SessionToken(env::var("USER_TOKEN")?);
	let botAuth:Authentication = Authentication::BotToken(env::var("BOT_TOKEN")?);

	let bot:Rive = Rive::new(botAuth).await?; // we have 2 auths, one for the bot that posts the links...
	let mut user:Rive = Rive::new(userAuth).await?; // ...and one for the normal user who scans servers for the links
	loop {
		while let Some(event) = user.gateway.next().await {
			let event:ServerEvent = event?;

			user.update(&event); // updating info & cache for the observer user

			if let ServerEvent::Message(message) = event {
				match user.cache.channel(&message.channel) {
					Some(channel) => {
						// Scan for invites in server channels
						if let Channel::TextChannel{server, ..} = channel.value() {
							// server != "Bottomless Pit" && author != Observer && author != Bot && channel.isValid()
							if server != "01HQPN9X7TJPYR25X5XZQH5AAR" && message.author != "01HGZWD37QKDAHVRNXEMJWFX5S" && message.author != "01HQRRRHXJ55FZ21DVEWHYJF89" && !INVALID_CHANNELS.contains(&message.channel.as_str()) {
								if let Some(msg) = message.content {
									tryIndexInviteFromMessage(&linkRipper, msg, &bot, &user).await;
								}
							}
						} else if let Channel::DirectMessage{id, ..} = channel.value() {
							if message.author == OWNER_ID {
								if let Some(msg) = message.content {
									if msg.starts_with("observer ") {
										let cmd = msg.trim_start_matches("observer ");
										if cmd.starts_with("ping") {
											informOwner(&user, SendMessageData {
												content: Some("Aye, pong!".to_owned()),
												..Default::default()
											}).await;
										} else if cmd.starts_with("join") {
											if let Some(id) = ripPatternFromText(&linkIdRipper, cmd.to_string()).await {
												match user.http.join_invite(id.clone()).await {
													Ok(joinDat) => {
														informOwner(&user, SendMessageData {
															content: Some("Aye, observing...".to_owned()),
															..Default::default()
														}).await;
														logToNotes(&user, SendMessageData{
															content: Some("Started listening via invite: INVID".replace("INVID", &id)),
															..Default::default()
														}).await;
													}
													Err(e) => {
														informOwner(&user, SendMessageData {
															content: Some("Nah! Can't join!".to_owned()),
															..Default::default()
														}).await;
														logToNotes(&user, SendMessageData{
															content: Some("Failed to join invite: INVID".replace("INVID", &id)),
															..Default::default()
														}).await;
													}
												}
											}
										} else if cmd.starts_with("leave") {
											if let Some(id) = ripPatternFromText(&idRipper, cmd.to_string()).await {
												match user.http.delete_or_leave_server(id.clone()).await {
													Ok(joinDat) => {
														informOwner(&user, SendMessageData {
															content: Some("Aye, stopped observing.".to_owned()),
															..Default::default()
														}).await;
														logToNotes(&user, SendMessageData{
															content: Some("Stopped listening in: INVID".replace("INVID", &id)),
															..Default::default()
														}).await;
													}
													Err(e) => {
														informOwner(&user, SendMessageData {
															content: Some("Nah! I'm stuck!".to_owned()),
															..Default::default()
														}).await;
														logToNotes(&user, SendMessageData{
															content: Some("Failed to leave: INVID".replace("INVID", &id)),
															..Default::default()
														}).await;
													}
												}
											}
										} else if cmd.starts_with("forceIndex") && tryIndexInviteFromMessage(&linkRipper, cmd.to_string(), &bot, &user).await {
											informOwner(&user, SendMessageData {
												content: Some("Aye, server indexed.".to_owned()),
												..Default::default()
											}).await;
										}
									}
								}
							}
						}
					}
					None => {
						println!("A message was found in a null channel! Reporting...");
						logToNotes(&user, SendMessageData{
							content: Some("A message was found in a null channel!".to_string()),
							..Default::default()
						}).await;
					}
				}
			} /*else if let ServerEvent::ServerMemberJoin(evt) = event {
			let _ = user.http.edit_member(evt.id, evt.user, EditMemberData {
				nickname: Some("𐑞y𐑞".to_string()),
				..Default::default()
			}).await;
		}*/
		}
		println!("Lost connection, reconnecting...");
	}
}

// Logs a message to Saved Notes
async fn logToNotes(user:&Rive, msg:SendMessageData) {
	match user.http.fetch_account().await {
		Ok(acc) => {
			match user.http.open_direct_message(&acc.id).await {
				Ok(channel) => {
					if let Channel::SavedMessages{id, ..} = channel {
						if let Err(e) = user.http.send_message(id, msg.clone()).await {
							println!("Error: Failed to send message in Saved Notes: {:?}", e)
						}
					}
				}
				Err(e) => {
					println!("Error: Failed to open Saved Notes: {:?}", e);
				}
			}
		}
		Err(e) => println!("Error whilst trying to fetch observer account: {:?}", e)
	}
}
// Sends a message to the bot owner
async fn informOwner(user:&Rive, msg:SendMessageData) {
	match user.http.open_direct_message(OWNER_ID).await {
		Ok(channel) => {
			if let Channel::DirectMessage{id, ..} = channel {
				if let Err(e) = user.http.send_message(id, msg).await {
					println!("Error: Failed to send message to owner: {:?}", e)
				}
			}
		}
		Err(e) => {
			println!("Error: Failed to open owner DMs: {:?}", e);
		}
	}
}

async fn tryIndexInviteFromMessage(ripper:&Regex, msg:String, bot:&Rive, user:&Rive) -> bool {
	return match ripper.captures(&msg) {
		Some(invites) => {
			println!("Indexing invite link: {}", &invites["link"]);
			let data = SendMessageData {
				content: Some(invites["link"].to_owned()),
				..Default::default()
			};
			if let Err(e) = bot.http.send_message("01HQS9NN019MR8RHN2VHG259WB", data.clone()).await {
				println!("Error whilst trying to finish indexing: {:?}", e);
				let data = SendMessageData {
					content: Some("Failed to index: LINK".replace("LINK", &invites["link"])),
					..Default::default()
				};
				logToNotes(user, data.clone()).await;
				informOwner(user, data).await;
				return false; // error!
			}
			true // return true, as things went ok
		}
		None => false // didnt find an invite
	};
}
async fn ripPatternFromText(ripper:&Regex, msg:String) -> Option<String> {
	return ripper.captures(&msg).map(|invites| invites["link"].to_owned());
}