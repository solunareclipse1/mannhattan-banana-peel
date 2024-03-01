#![allow(non_snake_case)]
#![allow(clippy::needless_return)]
#![allow(clippy::collapsible_match)]

use regex::Regex;
use rive::prelude::*;
use std::{env, error::Error};
use lazy_static::lazy_static;
use rive::prelude::HttpError::{Api, HttpRequest, Serialization};

const INVALID_CHANNELS:[&str; 3] = [
	"01HBPSCHW964KDCGF7RC4HCCSR", // Barry, 63: test2 (deny)
	"01FD53QCD84PX7D2GBV5SBE09N", // Revolt: Submit to Discover
	"01HC0P7QBKYPHH97D1ZMD9E9BC", // Revolt: Looking for Group
];


const OWNER_ID:&str = "01GV7GN0H4JT7EWG5GY64RA2VV";
const INDEX_SERVER:&str = "01HQPN9X7TJPYR25X5XZQH5AAR";
const INDEX_CATEGORY:&str = "Pit Directory";

// fuck you, close enough
struct MutableStatic {
	USER:Rive,
	BOT:Rive,
	USERID:String,
	BOTID:String
}

lazy_static! {
	// version with HTTPS in front of it, but that isnt needed for the links
	//let inviteRipper:Regex = Regex::new(r"(?<link>https:\/\/rvlt.gg\/[\w|\d]{8})").unwrap();
	static ref LINK_RIPPER:Regex = Regex::new(r"(?<link>rvlt.gg\/[\w|\d]{8})").unwrap();
	static ref LINK_ID_RIPPER:Regex = Regex::new(r"rvlt.gg\/(?<link>[\w|\d]{8})").unwrap();
	static ref ULID_RIPPER:Regex = Regex::new(r"(?<link>[\dA-Z]{26})").unwrap();
	
	//static ref USER_AUTH:Authentication = Authentication::SessionToken(env::var("USER_TOKEN").unwrap());
	//static ref BOT_AUTH:Authentication = Authentication::BotToken(env::var("BOT_TOKEN").unwrap());
	//static ref BOT:Rive = {
	//	tokio::runtime::Runtime::new().unwrap().block_on(async {
	//		Rive::new(Authentication::BotToken(env::var("BOT_TOKEN").unwrap())).await.unwrap()
	//	})
	//};
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
	let u:Rive = Rive::new(Authentication::SessionToken(env::var("USER_TOKEN").unwrap())).await.unwrap();
	let b:Rive = Rive::new(Authentication::BotToken(env::var("BOT_TOKEN").unwrap())).await.unwrap();
	let userAcc:AccountInfo = u.http.fetch_account().await.unwrap();
	let botAcc = b.http.fetch_self().await.unwrap();
	let mut S = MutableStatic {
		USER: u,
		BOT: b,
		USERID: userAcc.id,
		BOTID: botAcc.id
	};
	
	loop {
		while let Some(event) = S.USER.gateway.next().await {
			if let Ok(event) = event {
				S.USER.update(&event); // updating info & cache for the observer user
				if let ServerEvent::Message(message) = event {
					match S.USER.cache.channel(&message.channel) {
						Some(channel) => {
							let res = match channel.value() {
								Channel::TextChannel { server, .. } => {
									checkForInvites(&S, server, &message).await
								}
								Channel::DirectMessage { id, .. } => {
									if message.author == OWNER_ID {
										handleCommand(&S, id, &message).await
									} else if message.author == S.USERID {
										Ok(())
									} else {
										Err("Not the owner".into())
									}
								}
								_ => Err("Not a valid channel source".into()),
							};
							if res.is_err() {
								println!("{:?}", res);
							}
						}
						None => {
							println!("A message was found in a null channel! Reporting...");
							logToNotes(&S, SendMessageData {
								content: Some("A message was found in a null channel!".to_string()),
								..Default::default()
							}).await;
						}
					}
				}
			}
			//println!("Lost connection, reconnecting...");
		}
	}
}

// Scan for invites in server channels
async fn checkForInvites(S:&MutableStatic, server:&str, message:&Message) -> Result<(), Box<dyn Error>> {
	// server != "Bottomless Pit" && author != Observer && author != Bot && channel.isValid()
	if server != INDEX_SERVER
			&& message.author != S.USERID
			&& message.author != S.BOTID
			&& !INVALID_CHANNELS.contains(&message.channel.as_str()) {
		if let Some(msg) = &message.content {
			tryIndexInviteFromMessage(S, msg).await?
		} else {
			return Err("No message!".into());
		}
	}
	return Err("Blacklisted Source".into());
}

async fn handleCommand(S:&MutableStatic, id:&str, message:&Message) -> Result<(), Box<dyn Error>> {
	if let Some(cmd) = &message.content {
		if cmd.starts_with("ping") {
			informOwner(S, SendMessageData {
				content: Some("Aye, pong!".to_owned()),
				..Default::default()
			}).await;
		} else if cmd.starts_with("observe") {
			if let Some(id) = ripPatternFromText(S, &LINK_ID_RIPPER, cmd.to_string()).await {
				match S.USER.http.join_invite(id.clone()).await {
					Ok(joinDat) => {
						informOwner(S, SendMessageData {
							content: Some("Aye, observing...".to_owned()),
							..Default::default()
						}).await;
						logToNotes(S, SendMessageData {
							content: Some("Started listening via invite: INVID".replace("INVID", &id)),
							..Default::default()
						}).await;
					}
					Err(e) => {
						informOwner(S, SendMessageData {
							content: Some("Nah! Can't join!".to_owned()),
							..Default::default()
						}).await;
						logToNotes(S, SendMessageData {
							content: Some("Failed to join invite: INVID".replace("INVID", &id)),
							..Default::default()
						}).await;
					}
				}
			}
		} else if cmd.starts_with("leave") {
			if let Some(id) = ripPatternFromText(S, &ULID_RIPPER, cmd.to_string()).await {
				match S.USER.http.delete_or_leave_server(id.clone()).await {
					Ok(joinDat) => {
						informOwner(S, SendMessageData {
							content: Some("Aye, stopped observing.".to_owned()),
							..Default::default()
						}).await;
						logToNotes(S, SendMessageData {
							content: Some("Stopped listening in: INVID".replace("INVID", &id)),
							..Default::default()
						}).await;
					}
					Err(e) => {
						match e {
							Serialization(_) => {
								informOwner(S, SendMessageData {
									content: Some("Nah! I'm stuck!".to_owned()),
									..Default::default()
								}).await;
								logToNotes(S, SendMessageData {
									content: Some("Failed to leave: INVID".replace("INVID", &id)),
									..Default::default()
								}).await;
							}
							HttpRequest(_) => {
								informOwner(S, SendMessageData {
									content: Some("Aye, I think that worked.".to_owned()),
									..Default::default()
								}).await;
								logToNotes(S, SendMessageData {
									content: Some("Left: INVID".replace("INVID", &id)),
									..Default::default()
								}).await;
							}
							Api(_) => {
								informOwner(S, SendMessageData {
									content: Some("Nah! I'm not there!".to_owned()),
									..Default::default()
								}).await;
								logToNotes(S, SendMessageData {
									content: Some("Already left: INVID".replace("INVID", &id)),
									..Default::default()
								}).await;
							}
						}
						//informOwner(SendMessageData {
						//	content: Some("Nah! I'm stuck!".to_owned()),
						//	..Default::default()
						//}).await;
						//logToNotes(SendMessageData {
						//	content: Some("Failed to leave: INVID".replace("INVID", &id)),
						//	..Default::default()
						//}).await;
					}
				}
			}
		} else if cmd.starts_with("forceIndex") && tryIndexInviteFromMessage(S, cmd).await.is_ok() {
			informOwner(S, SendMessageData {
				content: Some("Aye, server indexed.".to_owned()),
				..Default::default()
			}).await;
		}
	}
	Ok(())
}

// Logs a message to Saved Notes
async fn logToNotes(S:&MutableStatic, msg: SendMessageData) {
	match S.USER.http.fetch_account().await {
		Ok(acc) => match S.USER.http.open_direct_message(&acc.id).await {
			Ok(channel) => {
				if let Channel::SavedMessages { id, .. } = channel {
					if let Err(e) = S.USER.http.send_message(id, msg.clone()).await {
						println!("Error: Failed to send message in Saved Notes: {:?}", e)
					}
				}
			}
			Err(e) => {
				println!("Error: Failed to open Saved Notes: {:?}", e);
			}
		},
		Err(e) => println!("Error whilst trying to fetch observer account: {:?}", e),
	}
}
// Sends a message to the bot owner
async fn informOwner(S:&MutableStatic, msg: SendMessageData) {
	match S.USER.http.open_direct_message(OWNER_ID).await {
		Ok(channel) => {
			if let Channel::DirectMessage { id, .. } = channel {
				if let Err(e) = S.USER.http.send_message(id, msg).await {
					println!("Error: Failed to send message to owner: {:?}", e)
				}
			}
		}
		Err(e) => {
			println!("Error: Failed to open owner DMs: {:?}", e);
		}
	}
}

async fn tryIndexInviteFromMessage(S:&MutableStatic, msg:&str) -> Result<(), Box<dyn Error>> {
	return match LINK_RIPPER.captures(msg) {
		Some(invites) => {
			println!("Indexing invite link: {}", &invites["link"]);
			let data = SendMessageData {
				content: Some(invites["link"].to_owned()),
				..Default::default()
			};
			//if let Ok(Invite::Server{server_id, server_name, server_icon, ..}) = S.USER.http.fetch_invite(&invites["link"]).await {
			match S.USER.http.fetch_invite(&invites["link"]).await {
				Ok(Invite::Server{server_id, server_name, server_icon, ..}) => {
					match S.BOT.http.fetch_server(INDEX_SERVER).await {
						Ok(srv) => {
							if let Some(categories) = srv.categories {
								for cat in categories {
									if cat.title == INDEX_CATEGORY {
										for chan in cat.channels {
											match S.BOT.http.fetch_channel(chan).await {
												Ok(c) => {
													if let Channel::TextChannel{description, ..} = c {
														if let Some(desc) = description {
															if desc == server_id {
																match S.BOT.http.send_message(desc, data.clone()).await {
																	Ok(_) => return Ok(()),
																	Err(e) => {
																		println!("Error whilst trying to finish indexing in an existing channel: {:?}", e);
																		let data = SendMessageData {
																			content: Some("Failed to index existing: LINK".replace("LINK", &invites["link"])),
																			..Default::default()
																		};
																		logToNotes(S, data.clone()).await;
																		informOwner(S, data).await;
																		return Err(Box::new(e));
																	}
																}
															}
														}
													}
												}
												Err(e) => {
													println!("Error whilst trying to fetch index server while indexing: {:?}", e);
													let data = SendMessageData {
														content: Some("Failed to get index server when fetching: LINK".replace("LINK", &invites["link"])),
														..Default::default()
													};
													logToNotes(S, data.clone()).await;
													informOwner(S, data).await;
													return Err(Box::new(e));
												}
											}
										}
										// Need to create a new channel
										match S.BOT.http.create_channel(&server_id, CreateChannelData{
											channel_type: ChannelType::Text,
											name: server_name.clone(),
											description: Some(server_id.clone()),
											nsfw: Some(false),
										}).await {
											Ok(chan) => {
												if let Channel::TextChannel{id, ..} = chan {
													match S.BOT.http.send_message(id, data.clone()).await {
														Ok(_) => return Ok(()),
														Err(e) => {
															println!("Error whilst trying to finish indexing into a new channel: {:?}", e);
															let data = SendMessageData {
																content: Some("Failed to index new: LINK".replace("LINK", &invites["link"])),
																..Default::default()
															};
															logToNotes(S, data.clone()).await;
															informOwner(S, data).await;
															return Err(Box::new(e));
														}
													}
												}
											}
											Err(e) => {
												println!("Error whilst trying to create new channel for: {:?}", e);
												let data = SendMessageData {
													content: Some("Failed to create new index for: LINK".replace("LINK", &invites["link"])),
													..Default::default()
												};
												logToNotes(S, data.clone()).await;
												informOwner(S, data).await;
												return Err(Box::new(e));
											}
										}
									}
								}
							} else {
								println!("Error: No categories in index server! Reporting...");
								let data = SendMessageData {
									content: Some("Failed to index because idx server has no categories: LINK".replace("LINK", &invites["link"])),
									..Default::default()
								};
								logToNotes(S, data.clone()).await;
								informOwner(S, data).await;
								return Err("No categories found in the Index server!".into());
							}
						},
						Err(e) => {
							println!("Error whilst trying to fetch index server while indexing: {:?}", e);
							let data = SendMessageData {
								content: Some("Failed to get index server when fetching: LINK".replace("LINK", &invites["link"])),
								..Default::default()
							};
							logToNotes(S, data.clone()).await;
							informOwner(S, data).await;
							return Err(Box::new(e));
						}
					}
				}
				Err(e) => {
					println!("Error whilst fetching invite data: {:?}", e);
					let data = SendMessageData {
						content: Some("Failed to fetch invite: LINK".replace("LINK", &invites["link"])),
						..Default::default()
					};
					logToNotes(S, data.clone()).await;
					informOwner(S, data).await;
					return Err(Box::new(e));
				}
				_ => return Err("Not a server invite!".into())
			}
			return Err("Fetching invite failed somehow?!".into());
			//match S.BOT.http.send_message("01HQS9NN019MR8RHN2VHG259WB", data.clone()).await {
			//	Ok(_) => Ok(()),
			//	Err(e) => {
			//		println!("Error whilst trying to finish indexing: {:?}", e);
			//		let data = SendMessageData {
			//			content: Some("Failed to index: LINK".replace("LINK", &invites["link"])),
			//			..Default::default()
			//		};
			//		logToNotes(S, data.clone()).await;
			//		informOwner(S, data).await;
			//		return Err(Box::new(e));
			//	}
			//}
		}
		None => return Err("No invites!".into()), // didnt find an invite
	};
}

async fn ripPatternFromText(S:&MutableStatic, ripper:&Regex, msg:String) -> Option<String> {
	return ripper
		.captures(&msg)
		.map(|invites| invites["link"].to_owned());
}