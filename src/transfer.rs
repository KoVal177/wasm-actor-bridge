//! Zero-copy transfer protocol between main thread and Web Worker.
//!
//! # Wire Format
//!
//! Each message is a 3-element JS `Array`:
//!
//! - `[0]` — correlation ID (`number` for RPC, `null` for fire-and-forget)
//! - `[1]` — typed message (structured-clone via `serde_wasm_bindgen`)
//! - `[2]` — `Uint8Array` (zero-copy transferred `ArrayBuffer`) or `null`

#[cfg(target_arch = "wasm32")]
mod wasm_impl {
    use js_sys::{Array, Uint8Array};
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use wasm_bindgen::JsCast;
    use web_sys::DedicatedWorkerGlobalScope;

    use crate::error::BridgeError;

    /// Raw wire data extracted from a `MessageEvent`.
    pub(crate) struct WireMessage {
        pub correlation_id: Option<u64>,
        pub payload: wasm_bindgen::JsValue,
        pub bytes: Option<Vec<u8>>,
    }

    /// Unpack a `MessageEvent` into raw wire components (no deserialization).
    pub(crate) fn unpack_wire(event: &web_sys::MessageEvent) -> Result<WireMessage, BridgeError> {
        let data: Array = event
            .data()
            .dyn_into()
            .map_err(|_| BridgeError::Serialisation("expected JS Array".into()))?;

        // Slot 0: correlation ID.
        let corr_js = data.get(0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let correlation_id =
            if corr_js.is_null() || corr_js.is_undefined() {
                None
            } else {
                Some(corr_js.as_f64().ok_or_else(|| {
                    BridgeError::Serialisation("correlation_id not a number".into())
                })? as u64)
            };

        // Slot 1: typed message (raw JsValue — caller deserializes).
        let payload = data.get(1);

        // Slot 2: binary sidecar.
        let js_bytes = data.get(2);
        let bytes = if js_bytes.is_null() || js_bytes.is_undefined() {
            None
        } else {
            let arr: Uint8Array = js_bytes
                .dyn_into()
                .map_err(|_| BridgeError::Serialisation("expected Uint8Array".into()))?;
            Some(arr.to_vec())
        };

        Ok(WireMessage {
            correlation_id,
            payload,
            bytes,
        })
    }

    /// Deserialize a raw `JsValue` into a typed message.
    pub(crate) fn deserialize_payload<T: DeserializeOwned>(
        js_value: wasm_bindgen::JsValue,
    ) -> Result<T, BridgeError> {
        serde_wasm_bindgen::from_value(js_value)
            .map_err(|e| BridgeError::Serialisation(format!("deserialize: {e}")))
    }

    /// Build a 3-slot wire array and post it.
    fn build_and_post(
        correlation_id: Option<u64>,
        js_msg: &wasm_bindgen::JsValue,
        bytes: Option<&[u8]>,
        post_fn: impl FnOnce(&Array, Option<&Array>) -> Result<(), wasm_bindgen::JsValue>,
    ) -> Result<(), BridgeError> {
        let msg = Array::new();

        // Slot 0: correlation ID.
        match correlation_id {
            #[allow(clippy::cast_precision_loss)] // IDs < 2^52: lossless.
            Some(id) => msg.push(&wasm_bindgen::JsValue::from_f64(id as f64)),
            None => msg.push(&wasm_bindgen::JsValue::NULL),
        };

        // Slot 1: typed message.
        msg.push(js_msg);

        // Slot 2: binary sidecar.
        if let Some(data) = bytes {
            let js_bytes = Uint8Array::from(data);
            msg.push(&js_bytes);

            let transfer = Array::new();
            transfer.push(&js_bytes.buffer());

            post_fn(&msg, Some(&transfer))
                .map_err(|e| BridgeError::PostMessage(format!("{e:?}")))?;
        } else {
            msg.push(&wasm_bindgen::JsValue::NULL);
            post_fn(&msg, None).map_err(|e| BridgeError::PostMessage(format!("{e:?}")))?;
        }

        Ok(())
    }

    /// Post a typed message + optional binary payload from Worker → Main.
    pub(crate) fn post_to_main<T: Serialize>(
        correlation_id: Option<u64>,
        message: &T,
        bytes: Option<&[u8]>,
    ) -> Result<(), BridgeError> {
        let js_msg = serde_wasm_bindgen::to_value(message)
            .map_err(|e| BridgeError::Serialisation(e.to_string()))?;

        let scope: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();

        build_and_post(
            correlation_id,
            &js_msg,
            bytes,
            |msg, transfer| match transfer {
                Some(t) => scope.post_message_with_transfer(msg, t),
                None => scope.post_message(msg),
            },
        )
    }

    /// Post a typed message + optional binary payload from Main → Worker.
    pub(crate) fn post_to_worker<T: Serialize>(
        worker: &web_sys::Worker,
        correlation_id: Option<u64>,
        message: &T,
        bytes: Option<&[u8]>,
    ) -> Result<(), BridgeError> {
        let js_msg = serde_wasm_bindgen::to_value(message)
            .map_err(|e| BridgeError::Serialisation(e.to_string()))?;

        build_and_post(
            correlation_id,
            &js_msg,
            bytes,
            |msg, transfer| match transfer {
                Some(t) => worker.post_message_with_transfer(msg, t),
                None => worker.post_message(msg),
            },
        )
    }

    /// Post a raw `JsValue` payload from Main → Worker without serialization.
    ///
    /// Used for cancel messages where the payload is `JsValue::NULL`.
    pub(crate) fn post_to_worker_raw(
        worker: &web_sys::Worker,
        correlation_id: Option<u64>,
        js_msg: &wasm_bindgen::JsValue,
        bytes: Option<&[u8]>,
    ) -> Result<(), BridgeError> {
        build_and_post(
            correlation_id,
            js_msg,
            bytes,
            |msg, transfer| match transfer {
                Some(t) => worker.post_message_with_transfer(msg, t),
                None => worker.post_message(msg),
            },
        )
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) use wasm_impl::*;
