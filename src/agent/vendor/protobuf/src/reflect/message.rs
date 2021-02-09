use descriptor::{DescriptorProto, FileDescriptorProto};
use descriptorx::find_message_by_rust_name;
use reflect::accessor::FieldAccessor;
use reflect::find_message_or_enum::find_message_or_enum;
use reflect::find_message_or_enum::MessageOrEnum;
use reflect::FieldDescriptor;
use std::collections::HashMap;
use std::marker;
use Message;

trait MessageFactory: Send + Sync + 'static {
    fn new_instance(&self) -> Box<dyn Message>;
}

struct MessageFactoryImpl<M>(marker::PhantomData<M>);

impl<M> MessageFactory for MessageFactoryImpl<M>
where
    M: 'static + Message + Default + Clone + PartialEq,
{
    fn new_instance(&self) -> Box<dyn Message> {
        let m: M = Default::default();
        Box::new(m)
    }
}

/// Dynamic message type
pub struct MessageDescriptor {
    full_name: String,
    proto: &'static DescriptorProto,
    factory: &'static dyn MessageFactory,
    fields: Vec<FieldDescriptor>,

    index_by_name: HashMap<String, usize>,
    index_by_number: HashMap<u32, usize>,
}

impl MessageDescriptor {
    /// Get underlying `DescriptorProto` object.
    pub fn get_proto(&self) -> &DescriptorProto {
        self.proto
    }

    /// Get a message descriptor for given message type
    pub fn for_type<M: Message>() -> &'static MessageDescriptor {
        M::descriptor_static()
    }

    fn compute_full_name(package: &str, path_to_package: &str, proto: &DescriptorProto) -> String {
        let mut full_name = package.to_owned();
        if path_to_package.len() != 0 {
            if full_name.len() != 0 {
                full_name.push('.');
            }
            full_name.push_str(path_to_package);
        }
        if full_name.len() != 0 {
            full_name.push('.');
        }
        full_name.push_str(proto.get_name());
        full_name
    }

    // Non-generic part of `new` is a separate function
    // to reduce code bloat from multiple instantiations.
    fn new_non_generic_by_rust_name(
        rust_name: &'static str,
        fields: Vec<Box<FieldAccessor + 'static>>,
        file: &'static FileDescriptorProto,
        factory: &'static dyn MessageFactory,
    ) -> MessageDescriptor {
        let proto = find_message_by_rust_name(file, rust_name);

        let mut field_proto_by_name = HashMap::new();
        for field_proto in proto.message.get_field() {
            field_proto_by_name.insert(field_proto.get_name(), field_proto);
        }

        let mut index_by_name = HashMap::new();
        let mut index_by_number = HashMap::new();
        for (i, f) in proto.message.get_field().iter().enumerate() {
            index_by_number.insert(f.get_number() as u32, i);
            index_by_name.insert(f.get_name().to_string(), i);
        }

        let mut full_name = file.get_package().to_string();
        if full_name.len() > 0 {
            full_name.push('.');
        }
        full_name.push_str(proto.message.get_name());

        MessageDescriptor {
            full_name: full_name,
            proto: proto.message,
            factory,
            fields: fields
                .into_iter()
                .map(|f| {
                    let proto = *field_proto_by_name.get(&f.name_generic()).unwrap();
                    FieldDescriptor::new(f, proto)
                })
                .collect(),
            index_by_name,
            index_by_number,
        }
    }

    // Non-generic part of `new` is a separate function
    // to reduce code bloat from multiple instantiations.
    fn new_non_generic_by_pb_name(
        protobuf_name_to_package: &'static str,
        fields: Vec<Box<FieldAccessor + 'static>>,
        file_descriptor_proto: &'static FileDescriptorProto,
        factory: &'static dyn MessageFactory,
    ) -> MessageDescriptor {
        let (path_to_package, proto) =
            match find_message_or_enum(file_descriptor_proto, protobuf_name_to_package) {
                (path_to_package, MessageOrEnum::Message(m)) => (path_to_package, m),
                (_, MessageOrEnum::Enum(_)) => panic!("not a message"),
            };

        let mut field_proto_by_name = HashMap::new();
        for field_proto in proto.get_field() {
            field_proto_by_name.insert(field_proto.get_name(), field_proto);
        }

        let mut index_by_name = HashMap::new();
        let mut index_by_number = HashMap::new();
        for (i, f) in proto.get_field().iter().enumerate() {
            index_by_number.insert(f.get_number() as u32, i);
            index_by_name.insert(f.get_name().to_string(), i);
        }

        MessageDescriptor {
            full_name: MessageDescriptor::compute_full_name(
                file_descriptor_proto.get_package(),
                &path_to_package,
                &proto,
            ),
            proto,
            factory,
            fields: fields
                .into_iter()
                .map(|f| {
                    let proto = *field_proto_by_name.get(&f.name_generic()).unwrap();
                    FieldDescriptor::new(f, proto)
                })
                .collect(),
            index_by_name,
            index_by_number,
        }
    }

    /// Construct a new message descriptor.
    ///
    /// This operation is called from generated code and rarely
    /// need to be called directly.
    #[doc(hidden)]
    #[deprecated(
        since = "2.12",
        note = "Please regenerate .rs files from .proto files to use newer APIs"
    )]
    pub fn new<M: 'static + Message + Default + Clone + PartialEq>(
        rust_name: &'static str,
        fields: Vec<Box<FieldAccessor + 'static>>,
        file: &'static FileDescriptorProto,
    ) -> MessageDescriptor {
        let factory = &MessageFactoryImpl(marker::PhantomData::<M>);
        MessageDescriptor::new_non_generic_by_rust_name(rust_name, fields, file, factory)
    }

    /// Construct a new message descriptor.
    ///
    /// This operation is called from generated code and rarely
    /// need to be called directly.
    #[doc(hidden)]
    pub fn new_pb_name<M: 'static + Message + Default + Clone + PartialEq>(
        protobuf_name_to_package: &'static str,
        fields: Vec<Box<FieldAccessor + 'static>>,
        file_descriptor_proto: &'static FileDescriptorProto,
    ) -> MessageDescriptor {
        let factory = &MessageFactoryImpl(marker::PhantomData::<M>);
        MessageDescriptor::new_non_generic_by_pb_name(
            protobuf_name_to_package,
            fields,
            file_descriptor_proto,
            factory,
        )
    }

    /// New empty message
    pub fn new_instance(&self) -> Box<dyn Message> {
        self.factory.new_instance()
    }

    /// Protobuf message name
    pub fn name(&self) -> &'static str {
        self.proto.get_name()
    }

    /// Fully qualified protobuf message name
    pub fn full_name(&self) -> &str {
        &self.full_name[..]
    }

    /// Message field descriptors.
    pub fn fields(&self) -> &[FieldDescriptor] {
        &self.fields
    }

    /// Find field by name
    pub fn field_by_name<'a>(&'a self, name: &str) -> &'a FieldDescriptor {
        // TODO: clone is weird
        let &index = self.index_by_name.get(&name.to_string()).unwrap();
        &self.fields[index]
    }

    /// Find field by number
    pub fn field_by_number<'a>(&'a self, number: u32) -> &'a FieldDescriptor {
        let &index = self.index_by_number.get(&number).unwrap();
        &self.fields[index]
    }
}
