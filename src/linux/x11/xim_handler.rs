// Modified to synchronize XIM initialization with native window focus.

use x11rb::protocol::{Event, xproto};
use xim::{AHashMap, AttributeName, Client, ClientError, ClientHandler, InputStyle};

pub enum XimCallbackEvent {
    XimReady,
    XimXEvent(x11rb::protocol::Event),
    XimPreeditEvent(xproto::Window, String),
    XimCommitEvent(xproto::Window, String),
}

pub struct XimHandler {
    state: XimState,
    pub last_callback_event: Option<XimCallbackEvent>,
}

#[derive(Clone, Copy)]
enum XimState {
    Opening,
    Open {
        im_id: u16,
    },
    Creating {
        im_id: u16,
        window: xproto::Window,
    },
    Ready {
        im_id: u16,
        ic_id: u16,
        window: xproto::Window,
    },
}

impl XimHandler {
    pub fn new() -> Self {
        Self {
            state: XimState::Opening,
            last_callback_event: None,
        }
    }

    pub fn context(&self) -> Option<(u16, u16, xproto::Window)> {
        match self.state {
            XimState::Ready {
                im_id,
                ic_id,
                window,
            } => Some((im_id, ic_id, window)),
            _ => None,
        }
    }

    pub fn sync_focus<C: Client>(
        &mut self,
        client: &mut C,
        window: Option<xproto::Window>,
        position: Option<xim::Point>,
    ) -> Result<(), ClientError> {
        let im_id = match self.state {
            XimState::Opening | XimState::Creating { .. } => return Ok(()),
            XimState::Open { im_id } => im_id,
            XimState::Ready {
                im_id,
                ic_id,
                window: context_window,
            } => {
                if window.is_none() {
                    return client.unset_focus(im_id, ic_id);
                }
                if window == Some(context_window) {
                    self.update_position(client, position)?;
                    return client.set_focus(im_id, ic_id);
                }
                client.destroy_ic(im_id, ic_id)?;
                self.state = XimState::Open { im_id };
                im_id
            }
        };
        let Some(window) = window else { return Ok(()) };
        let mut attributes = client
            .build_ic_attributes()
            .push(AttributeName::InputStyle, InputStyle::PREEDIT_CALLBACKS)
            .push(AttributeName::ClientWindow, window)
            .push(AttributeName::FocusWindow, window);
        if let Some(position) = position {
            attributes = attributes.nested_list(AttributeName::PreeditAttributes, |b| {
                b.push(AttributeName::SpotLocation, position);
            });
        }
        client.create_ic(im_id, attributes.build())?;
        self.state = XimState::Creating { im_id, window };
        Ok(())
    }

    pub fn update_position<C: Client>(
        &self,
        client: &mut C,
        position: Option<xim::Point>,
    ) -> Result<(), ClientError> {
        if let Some((im_id, ic_id, _)) = self.context()
            && let Some(position) = position
        {
            let attributes = client
                .build_ic_attributes()
                .nested_list(AttributeName::PreeditAttributes, |b| {
                    b.push(AttributeName::SpotLocation, position);
                })
                .build();
            client.set_ic_values(im_id, ic_id, attributes)?;
        }
        Ok(())
    }

    pub fn close_window<C: Client>(
        &mut self,
        client: &mut C,
        window: xproto::Window,
    ) -> Result<(), ClientError> {
        if let Some((im_id, ic_id, context_window)) = self.context()
            && context_window == window
        {
            client.destroy_ic(im_id, ic_id)?;
            self.state = XimState::Open { im_id };
        }
        Ok(())
    }

    fn callback_window(&self, im_id: u16, ic_id: u16) -> Option<xproto::Window> {
        self.context().and_then(|(context_im, context_ic, window)| {
            (context_im == im_id && context_ic == ic_id).then_some(window)
        })
    }
}

impl<C: Client<XEvent = xproto::KeyPressEvent>> ClientHandler<C> for XimHandler {
    fn handle_connect(&mut self, client: &mut C) -> Result<(), ClientError> {
        client.open("C")
    }

    fn handle_open(&mut self, client: &mut C, input_method_id: u16) -> Result<(), ClientError> {
        client.get_im_values(input_method_id, &[AttributeName::QueryInputStyle])
    }

    fn handle_get_im_values(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        _attributes: AHashMap<AttributeName, Vec<u8>>,
    ) -> Result<(), ClientError> {
        self.state = XimState::Open {
            im_id: input_method_id,
        };
        self.last_callback_event = Some(XimCallbackEvent::XimReady);
        Ok(())
    }

    fn handle_create_ic(
        &mut self,
        client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
    ) -> Result<(), ClientError> {
        let XimState::Creating { im_id, window } = self.state else {
            return client.destroy_ic(input_method_id, input_context_id);
        };
        if im_id != input_method_id {
            return client.destroy_ic(input_method_id, input_context_id);
        }
        self.state = XimState::Ready {
            im_id,
            ic_id: input_context_id,
            window,
        };
        self.last_callback_event = Some(XimCallbackEvent::XimReady);
        Ok(())
    }

    fn handle_commit(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
        text: &str,
    ) -> Result<(), ClientError> {
        if let Some(window) = self.callback_window(input_method_id, input_context_id) {
            self.last_callback_event = Some(XimCallbackEvent::XimCommitEvent(window, text.into()));
        }
        Ok(())
    }

    fn handle_forward_event(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
        _flag: xim::ForwardEventFlag,
        xev: C::XEvent,
    ) -> Result<(), ClientError> {
        if self
            .callback_window(input_method_id, input_context_id)
            .is_none()
        {
            return Ok(());
        }
        match xev.response_type {
            x11rb::protocol::xproto::KEY_PRESS_EVENT => {
                self.last_callback_event = Some(XimCallbackEvent::XimXEvent(Event::KeyPress(xev)));
            }
            x11rb::protocol::xproto::KEY_RELEASE_EVENT => {
                self.last_callback_event =
                    Some(XimCallbackEvent::XimXEvent(Event::KeyRelease(xev)));
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_close(&mut self, client: &mut C, _input_method_id: u16) -> Result<(), ClientError> {
        self.state = XimState::Opening;
        client.disconnect()
    }

    fn handle_preedit_draw(
        &mut self,
        _client: &mut C,
        input_method_id: u16,
        input_context_id: u16,
        _caret: i32,
        _chg_first: i32,
        _chg_len: i32,
        _status: xim::PreeditDrawStatus,
        preedit_string: &str,
        _feedbacks: Vec<xim::Feedback>,
    ) -> Result<(), ClientError> {
        // XIMReverse: 1, XIMPrimary: 8, XIMTertiary: 32: selected text
        // XIMUnderline: 2, XIMSecondary: 16: underlined text
        // XIMHighlight: 4: normal text
        // XIMVisibleToForward: 64, XIMVisibleToBackward: 128, XIMVisibleCenter: 256: text align position
        // XIMPrimary, XIMHighlight, XIMSecondary, XIMTertiary are not specified,
        // but interchangeable as above
        // Currently there's no way to support these.
        if let Some(window) = self.callback_window(input_method_id, input_context_id) {
            self.last_callback_event = Some(XimCallbackEvent::XimPreeditEvent(
                window,
                preedit_string.into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        rc::Rc,
        time::{Duration, Instant},
    };
    use x11rb::{connection::Connection, protocol::xproto::ConnectionExt, xcb_ffi::XCBConnection};
    use xim::x11rb::X11rbClient;

    // The XIM crate's client builder is private, so use the real protocol adapter.
    // Run explicitly on an isolated X11 display with a running XIM server.
    fn check_focus_order(focus_first: bool) {
        let (connection, screen) = XCBConnection::connect(None).unwrap();
        let connection = Rc::new(connection);
        let window = connection.generate_id().unwrap();
        connection
            .create_window(
                0,
                window,
                connection.setup().roots[screen].root,
                0,
                0,
                320,
                200,
                0,
                xproto::WindowClass::INPUT_OUTPUT,
                0,
                &xproto::CreateWindowAux::default(),
            )
            .unwrap()
            .check()
            .unwrap();
        let mut client = X11rbClient::init(connection.clone(), screen, None).unwrap();
        let mut handler = XimHandler::new();
        let mut focus = focus_first.then_some(window);
        handler.sync_focus(&mut client, focus, None).unwrap();
        assert!(handler.context().is_none());
        let deadline = Instant::now() + Duration::from_secs(3);
        while handler.context().is_none() {
            assert!(Instant::now() < deadline, "XIM did not become ready");
            if let Some(event) = connection.poll_for_event().unwrap() {
                client.filter_event(&event, &mut handler).unwrap();
                if let Some(XimCallbackEvent::XimReady) = handler.last_callback_event.take() {
                    handler.sync_focus(&mut client, focus, None).unwrap();
                }
                if !focus_first && matches!(handler.state, XimState::Open { .. }) {
                    // Handshake completed with no focused window and no IC.
                    assert!(handler.context().is_none());
                    focus = Some(window);
                    handler.sync_focus(&mut client, focus, None).unwrap();
                }
            } else {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        let context = handler.context().unwrap();
        assert!(context.0 != 0 && context.1 != 0);
        assert_eq!(context.2, window);
        for _ in 0..20 {
            handler.sync_focus(&mut client, None, None).unwrap();
            handler.sync_focus(&mut client, Some(window), None).unwrap();
            assert_eq!(handler.context(), Some(context));
        }
        handler.close_window(&mut client, window).unwrap();
        assert!(handler.context().is_none());
        connection.destroy_window(window).unwrap().check().unwrap();
        connection.flush().unwrap();
    }

    #[test]
    #[ignore = "requires an isolated X11 display with a running XIM server"]
    fn focus_before_handshake_creates_one_reusable_context() {
        check_focus_order(true);
    }

    #[test]
    #[ignore = "requires an isolated X11 display with a running XIM server"]
    fn handshake_before_focus_waits_for_a_native_window() {
        check_focus_order(false);
    }
}
