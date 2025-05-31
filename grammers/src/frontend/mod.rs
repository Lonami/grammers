// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod generated;

use slint::{ComponentHandle as _, Image, ModelRc, VecModel};
use std::rc::Rc;

use crate::bridge::{BackendMessage, FrontendContext, FrontendMessage};
use generated::{
    AppWindow, DialogItemModel, LoginScreenGlobal, LoginStepEnum, MainScreenGlobal,
    MessageItemModel, ScreenEnum,
};

pub fn main(mut context: FrontendContext) -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;

    // Initialize models
    let dialog_items_model = Rc::new(VecModel::from(Vec::<DialogItemModel>::new()));
    let message_items_model = Rc::new(VecModel::from(Vec::<MessageItemModel>::new()));

    app.global::<MainScreenGlobal>()
        .set_dialogs(ModelRc::from(dialog_items_model.clone()));

    app.global::<MainScreenGlobal>()
        .set_messages(ModelRc::from(message_items_model.clone()));

    // Register callbacks
    app.global::<LoginScreenGlobal>().on_request_continue({
        let app_handle = app.as_weak();
        let sender = context.frontend_sender.clone();
        move |value| {
            let app = app_handle.unwrap();
            match app.global::<LoginScreenGlobal>().get_step() {
                LoginStepEnum::Phone => {
                    let _ = sender.send(FrontendMessage::LoginPhone(value.into()));
                }
                LoginStepEnum::Code => {
                    let _ = sender.send(FrontendMessage::LoginCode(value.into()));
                }
                LoginStepEnum::Password => {
                    let _ = sender.send(FrontendMessage::LoginPassword(value.into()));
                }
            }
        }
    });

    app.global::<MainScreenGlobal>().on_request_messages({
        let app_handle = app.as_weak();
        let sender = context.frontend_sender.clone();
        move || {
            let app = app_handle.unwrap();
            let _ = sender.send(FrontendMessage::FetchMessages {
                peer: app
                    .global::<MainScreenGlobal>()
                    .get_selected_dialog_id()
                    .into(),
            });
        }
    });

    app.global::<MainScreenGlobal>().on_request_send_message({
        let app_handle = app.as_weak();
        let sender = context.frontend_sender.clone();
        move |message| {
            let app = app_handle.unwrap();
            let _ = sender.send(FrontendMessage::SendMessage {
                peer: app
                    .global::<MainScreenGlobal>()
                    .get_selected_dialog_id()
                    .into(),
                message: message.into(),
            });
        }
    });

    // Loop-process backend messages
    slint::spawn_local({
        let app_handle = app.as_weak();
        async move {
            while let Some((message, app)) = context
                .frontend_receiver
                .recv()
                .await
                .zip(app_handle.upgrade())
            {
                match message {
                    BackendMessage::NeedLoginPhone => {
                        app.set_screen(ScreenEnum::Login);
                        app.global::<LoginScreenGlobal>()
                            .set_step(LoginStepEnum::Phone);
                    }
                    BackendMessage::NeedLoginCode => {
                        app.set_screen(ScreenEnum::Login);
                        app.global::<LoginScreenGlobal>()
                            .set_step(LoginStepEnum::Code);
                    }
                    BackendMessage::NeedLoginPassword { hint } => {
                        app.set_screen(ScreenEnum::Login);
                        app.global::<LoginScreenGlobal>()
                            .set_step(LoginStepEnum::Password);
                        app.global::<LoginScreenGlobal>()
                            .set_password_hint(hint.unwrap_or_default().into());
                    }
                    BackendMessage::LoginSuccess => {
                        app.set_screen(ScreenEnum::Main);
                    }
                    BackendMessage::Dialogs(dialogs) => {
                        dialog_items_model.set_vec(
                            dialogs
                                .into_iter()
                                .map(|dialog| DialogItemModel {
                                    id: dialog.peer().id().into(),
                                    last_message_read: false,
                                    last_message_sender_name: dialog
                                        .last_message
                                        .as_ref()
                                        .and_then(|m| m.sender())
                                        .and_then(|s| s.name().map(String::from))
                                        .unwrap_or_default()
                                        .into(),
                                    last_message_sent: dialog
                                        .last_message
                                        .as_ref()
                                        .map(|m| m.outgoing())
                                        .unwrap_or_default(),
                                    last_message_text: dialog
                                        .last_message
                                        .as_ref()
                                        .map(|m| String::from(m.text()))
                                        .unwrap_or_default()
                                        .into(),
                                    last_message_time: dialog
                                        .last_message
                                        .as_ref()
                                        .map(|m| m.date().format("%H:%M").to_string())
                                        .unwrap_or_default()
                                        .into(),
                                    name: dialog
                                        .peer()
                                        .name()
                                        .map(String::from)
                                        .unwrap_or_default()
                                        .into(),
                                    picture_thumbnail: Image::default(),
                                })
                                .collect::<Vec<_>>(),
                        );
                    }
                    BackendMessage::Messages(messages) => {
                        let should_set = messages.get(0).is_none_or(|message| {
                            generated::ChatId::from(message.peer_ref().id)
                                == app.global::<MainScreenGlobal>().get_selected_dialog_id()
                        });

                        let messages = if should_set { messages } else { Vec::new() };

                        message_items_model.set_vec(
                            messages
                                .into_iter()
                                .map(|message| MessageItemModel {
                                    id: message.id(),
                                    read: !message.outgoing(),
                                    sender_name: message
                                        .sender()
                                        .and_then(|s| s.name().map(String::from))
                                        .unwrap_or_default()
                                        .into(),
                                    sender_picture_thumbnail: Image::default(),
                                    sent: message.outgoing(),
                                    text: message.text().into(),
                                    time: message.date().format("%H:%M").to_string().into(),
                                })
                                .collect::<Vec<_>>(),
                        );
                    }
                }
            }

            let _ = slint::quit_event_loop();
        }
    })
    .expect("frontend task to start");

    app.run()
}
