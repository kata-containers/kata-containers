use crate::parse_from_bytes;
use crate::reflect::MessageDescriptor;
use crate::well_known_types::Any;
use crate::Message;
use crate::ProtobufResult;

impl Any {
    fn type_url(type_url_prefix: &str, descriptor: &MessageDescriptor) -> String {
        format!("{}/{}", type_url_prefix, descriptor.full_name())
    }

    fn get_type_name_from_type_url(type_url: &str) -> Option<&str> {
        match type_url.rfind('/') {
            Some(i) => Some(&type_url[i + 1..]),
            None => None,
        }
    }

    /// Pack any message into `well_known_types::Any` value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use protobuf::Message;
    /// # use protobuf::ProtobufResult;
    /// use protobuf::well_known_types::Any;
    ///
    /// # fn the_test<MyMessage: Message>(message: &MyMessage) -> ProtobufResult<()> {
    /// let message: &MyMessage = message;
    /// let any = Any::pack(message)?;
    /// assert!(any.is::<MyMessage>());
    /// #   Ok(())
    /// # }
    /// ```
    pub fn pack<M: Message>(message: &M) -> ProtobufResult<Any> {
        Any::pack_dyn(message)
    }

    /// Pack any message into `well_known_types::Any` value.
    ///
    /// # Examples
    ///
    /// ```
    /// use protobuf::Message;
    /// # use protobuf::ProtobufResult;
    /// use protobuf::well_known_types::Any;
    ///
    /// # fn the_test(message: &dyn Message) -> ProtobufResult<()> {
    /// let message: &dyn Message = message;
    /// let any = Any::pack_dyn(message)?;
    /// assert!(any.is_dyn(message.descriptor()));
    /// #   Ok(())
    /// # }
    /// ```
    pub fn pack_dyn(message: &dyn Message) -> ProtobufResult<Any> {
        Any::pack_with_type_url_prefix(message, "type.googleapis.com")
    }

    fn pack_with_type_url_prefix(
        message: &dyn Message,
        type_url_prefix: &str,
    ) -> ProtobufResult<Any> {
        Ok(Any {
            type_url: Any::type_url(type_url_prefix, message.descriptor()),
            value: message.write_to_bytes()?,
            ..Default::default()
        })
    }

    /// Check if `Any` contains a message of given type.
    pub fn is<M: Message>(&self) -> bool {
        self.is_dyn(M::descriptor_static())
    }

    /// Check if `Any` contains a message of given type.
    pub fn is_dyn(&self, descriptor: &MessageDescriptor) -> bool {
        match Any::get_type_name_from_type_url(&self.type_url) {
            Some(type_name) => type_name == descriptor.full_name(),
            None => false,
        }
    }

    /// Extract a message from this `Any`.
    ///
    /// # Returns
    ///
    /// * `Ok(None)` when message type mismatch
    /// * `Err` when parse failed
    pub fn unpack<M: Message>(&self) -> ProtobufResult<Option<M>> {
        if !self.is::<M>() {
            return Ok(None);
        }
        Ok(Some(parse_from_bytes(&self.value)?))
    }

    /// Extract a message from this `Any`.
    ///
    /// # Returns
    ///
    /// * `Ok(None)` when message type mismatch
    /// * `Err` when parse failed
    pub fn unpack_dyn(
        &self,
        descriptor: &MessageDescriptor,
    ) -> ProtobufResult<Option<Box<dyn Message>>> {
        if !self.is_dyn(descriptor) {
            return Ok(None);
        }
        let mut message = descriptor.new_instance();
        message.merge_from_bytes(&self.value)?;
        message.check_initialized()?;
        Ok(Some(message))
    }
}
