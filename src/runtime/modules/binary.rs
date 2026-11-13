use boa_engine::{
    Context, JsNativeError, JsResult, JsValue,
    object::builtins::{JsArrayBuffer, JsUint8Array},
};

pub(super) fn view_bytes(view: &JsUint8Array, context: &mut Context) -> JsResult<Vec<u8>> {
    let offset = view.byte_offset(context)?;
    let length = view.byte_length(context)?;
    let backing = view
        .buffer(context)?
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("Buffer has no backing ArrayBuffer"))?;
    let backing = JsArrayBuffer::from_object(backing)?;
    let data = backing
        .data()
        .ok_or_else(|| JsNativeError::typ().with_message("Buffer is detached"))?;
    let bytes = data
        .get(offset..offset + length)
        .ok_or_else(|| JsNativeError::typ().with_message("Buffer view is out of bounds"))?;
    Ok(bytes.to_vec())
}

pub(super) fn array_buffer(bytes: &[u8], context: &mut Context) -> JsResult<JsValue> {
    let buffer = JsArrayBuffer::new(bytes.len(), context)?;
    buffer
        .data_mut()
        .expect("new ArrayBuffer is attached")
        .copy_from_slice(bytes);
    Ok(buffer.into())
}
