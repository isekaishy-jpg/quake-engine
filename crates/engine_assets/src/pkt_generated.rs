pub use root::*;

const _: () = ::planus::check_version_compatibility("planus-1.3.0");

/// The root namespace
///
/// Generated from these locations:
/// * File `crates\engine_assets\schema\pkt.fbs`
#[no_implicit_prelude]
#[allow(clippy::needless_lifetimes)]
mod root {
    /// The namespace `Pkt`
    ///
    /// Generated from these locations:
    /// * File `crates\engine_assets\schema\pkt.fbs`
    pub mod pkt {
        ///  One file in a container.
        ///
        /// Generated from these locations:
        /// * Table `Entry` in the file `crates\engine_assets\schema\pkt.fbs:6`
        #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
        pub struct Entry {
            ///  Its file id, from the content's table. Entries are sorted by this.
            pub fid: u32,
            ///  Its path.
            pub path: ::planus::alloc::string::String,
            ///  Where its bytes start, counted from the end of the index.
            pub offset: u64,
            ///  How long they are there.
            pub stored: u64,
            ///  How long the file is. If that's more than `stored`, it's squeezed.
            pub size: u64,
        }

        #[allow(clippy::derivable_impls)]
        impl ::core::default::Default for Entry {
            fn default() -> Self {
                Self {
                    fid: 0,
                    path: ::core::default::Default::default(),
                    offset: 0,
                    stored: 0,
                    size: 0,
                }
            }
        }

        impl Entry {
            /// Creates a [EntryBuilder] for serializing an instance of this table.
            #[inline]
            pub fn builder() -> EntryBuilder<()> {
                EntryBuilder(())
            }

            #[allow(clippy::too_many_arguments)]
            pub fn create(
                builder: &mut ::planus::Builder,
                field_fid: impl ::planus::WriteAsDefault<u32, u32>,
                field_path: impl ::planus::WriteAs<::planus::Offset<str>>,
                field_offset: impl ::planus::WriteAsDefault<u64, u64>,
                field_stored: impl ::planus::WriteAsDefault<u64, u64>,
                field_size: impl ::planus::WriteAsDefault<u64, u64>,
            ) -> ::planus::Offset<Self> {
                let prepared_fid = field_fid.prepare(builder, &0);
                let prepared_path = field_path.prepare(builder);
                let prepared_offset = field_offset.prepare(builder, &0);
                let prepared_stored = field_stored.prepare(builder, &0);
                let prepared_size = field_size.prepare(builder, &0);

                let mut table_writer: ::planus::table_writer::TableWriter<14> =
                    ::core::default::Default::default();
                if prepared_offset.is_some() {
                    table_writer.write_entry::<u64>(2);
                }
                if prepared_stored.is_some() {
                    table_writer.write_entry::<u64>(3);
                }
                if prepared_size.is_some() {
                    table_writer.write_entry::<u64>(4);
                }
                if prepared_fid.is_some() {
                    table_writer.write_entry::<u32>(0);
                }
                table_writer.write_entry::<::planus::Offset<str>>(1);

                unsafe {
                    table_writer.finish(builder, |object_writer| {
                        if let ::core::option::Option::Some(prepared_offset) = prepared_offset {
                            object_writer.write::<_, _, 8>(&prepared_offset);
                        }
                        if let ::core::option::Option::Some(prepared_stored) = prepared_stored {
                            object_writer.write::<_, _, 8>(&prepared_stored);
                        }
                        if let ::core::option::Option::Some(prepared_size) = prepared_size {
                            object_writer.write::<_, _, 8>(&prepared_size);
                        }
                        if let ::core::option::Option::Some(prepared_fid) = prepared_fid {
                            object_writer.write::<_, _, 4>(&prepared_fid);
                        }
                        object_writer.write::<_, _, 4>(&prepared_path);
                    });
                }
                builder.current_offset()
            }
        }

        impl ::planus::WriteAs<::planus::Offset<Entry>> for Entry {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Entry> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl ::planus::WriteAsOptional<::planus::Offset<Entry>> for Entry {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<Entry>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl ::planus::WriteAsOffset<Entry> for Entry {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Entry> {
                Entry::create(
                    builder,
                    self.fid,
                    &self.path,
                    self.offset,
                    self.stored,
                    self.size,
                )
            }
        }

        /// Builder for serializing an instance of the [Entry] type.
        ///
        /// Can be created using the [Entry::builder] method.
        #[derive(Debug)]
        #[must_use]
        pub struct EntryBuilder<State>(State);

        impl EntryBuilder<()> {
            /// Setter for the [`fid` field](Entry#structfield.fid).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn fid<T0>(self, value: T0) -> EntryBuilder<(T0,)>
            where
                T0: ::planus::WriteAsDefault<u32, u32>,
            {
                EntryBuilder((value,))
            }

            /// Sets the [`fid` field](Entry#structfield.fid) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn fid_as_default(self) -> EntryBuilder<(::planus::DefaultValue,)> {
                self.fid(::planus::DefaultValue)
            }
        }

        impl<T0> EntryBuilder<(T0,)> {
            /// Setter for the [`path` field](Entry#structfield.path).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn path<T1>(self, value: T1) -> EntryBuilder<(T0, T1)>
            where
                T1: ::planus::WriteAs<::planus::Offset<str>>,
            {
                let (v0,) = self.0;
                EntryBuilder((v0, value))
            }
        }

        impl<T0, T1> EntryBuilder<(T0, T1)> {
            /// Setter for the [`offset` field](Entry#structfield.offset).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn offset<T2>(self, value: T2) -> EntryBuilder<(T0, T1, T2)>
            where
                T2: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0, v1) = self.0;
                EntryBuilder((v0, v1, value))
            }

            /// Sets the [`offset` field](Entry#structfield.offset) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn offset_as_default(self) -> EntryBuilder<(T0, T1, ::planus::DefaultValue)> {
                self.offset(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2> EntryBuilder<(T0, T1, T2)> {
            /// Setter for the [`stored` field](Entry#structfield.stored).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn stored<T3>(self, value: T3) -> EntryBuilder<(T0, T1, T2, T3)>
            where
                T3: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0, v1, v2) = self.0;
                EntryBuilder((v0, v1, v2, value))
            }

            /// Sets the [`stored` field](Entry#structfield.stored) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn stored_as_default(self) -> EntryBuilder<(T0, T1, T2, ::planus::DefaultValue)> {
                self.stored(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3> EntryBuilder<(T0, T1, T2, T3)> {
            /// Setter for the [`size` field](Entry#structfield.size).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn size<T4>(self, value: T4) -> EntryBuilder<(T0, T1, T2, T3, T4)>
            where
                T4: ::planus::WriteAsDefault<u64, u64>,
            {
                let (v0, v1, v2, v3) = self.0;
                EntryBuilder((v0, v1, v2, v3, value))
            }

            /// Sets the [`size` field](Entry#structfield.size) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn size_as_default(self) -> EntryBuilder<(T0, T1, T2, T3, ::planus::DefaultValue)> {
                self.size(::planus::DefaultValue)
            }
        }

        impl<T0, T1, T2, T3, T4> EntryBuilder<(T0, T1, T2, T3, T4)> {
            /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [Entry].
            #[inline]
            pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<Entry>
            where
                Self: ::planus::WriteAsOffset<Entry>,
            {
                ::planus::WriteAsOffset::prepare(&self, builder)
            }
        }

        impl<
            T0: ::planus::WriteAsDefault<u32, u32>,
            T1: ::planus::WriteAs<::planus::Offset<str>>,
            T2: ::planus::WriteAsDefault<u64, u64>,
            T3: ::planus::WriteAsDefault<u64, u64>,
            T4: ::planus::WriteAsDefault<u64, u64>,
        > ::planus::WriteAs<::planus::Offset<Entry>> for EntryBuilder<(T0, T1, T2, T3, T4)>
        {
            type Prepared = ::planus::Offset<Entry>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Entry> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl<
            T0: ::planus::WriteAsDefault<u32, u32>,
            T1: ::planus::WriteAs<::planus::Offset<str>>,
            T2: ::planus::WriteAsDefault<u64, u64>,
            T3: ::planus::WriteAsDefault<u64, u64>,
            T4: ::planus::WriteAsDefault<u64, u64>,
        > ::planus::WriteAsOptional<::planus::Offset<Entry>>
            for EntryBuilder<(T0, T1, T2, T3, T4)>
        {
            type Prepared = ::planus::Offset<Entry>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<Entry>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl<
            T0: ::planus::WriteAsDefault<u32, u32>,
            T1: ::planus::WriteAs<::planus::Offset<str>>,
            T2: ::planus::WriteAsDefault<u64, u64>,
            T3: ::planus::WriteAsDefault<u64, u64>,
            T4: ::planus::WriteAsDefault<u64, u64>,
        > ::planus::WriteAsOffset<Entry> for EntryBuilder<(T0, T1, T2, T3, T4)>
        {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Entry> {
                let (v0, v1, v2, v3, v4) = &self.0;
                Entry::create(builder, v0, v1, v2, v3, v4)
            }
        }

        /// Reference to a deserialized [Entry].
        #[derive(Copy, Clone)]
        pub struct EntryRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

        impl<'a> EntryRef<'a> {
            /// Getter for the [`fid` field](Entry#structfield.fid).
            #[inline]
            pub fn fid(&self) -> ::planus::Result<u32> {
                ::core::result::Result::Ok(self.0.access(0, "Entry", "fid")?.unwrap_or(0))
            }

            /// Getter for the [`path` field](Entry#structfield.path).
            #[inline]
            pub fn path(&self) -> ::planus::Result<&'a ::core::primitive::str> {
                self.0.access_required(1, "Entry", "path")
            }

            /// Getter for the [`offset` field](Entry#structfield.offset).
            #[inline]
            pub fn offset(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(self.0.access(2, "Entry", "offset")?.unwrap_or(0))
            }

            /// Getter for the [`stored` field](Entry#structfield.stored).
            #[inline]
            pub fn stored(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(self.0.access(3, "Entry", "stored")?.unwrap_or(0))
            }

            /// Getter for the [`size` field](Entry#structfield.size).
            #[inline]
            pub fn size(&self) -> ::planus::Result<u64> {
                ::core::result::Result::Ok(self.0.access(4, "Entry", "size")?.unwrap_or(0))
            }
        }

        impl<'a> ::core::fmt::Debug for EntryRef<'a> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut f = f.debug_struct("EntryRef");
                f.field("fid", &self.fid());
                f.field("path", &self.path());
                f.field("offset", &self.offset());
                f.field("stored", &self.stored());
                f.field("size", &self.size());
                f.finish()
            }
        }

        impl<'a> ::core::convert::TryFrom<EntryRef<'a>> for Entry {
            type Error = ::planus::Error;

            #[allow(unreachable_code)]
            fn try_from(value: EntryRef<'a>) -> ::planus::Result<Self> {
                ::core::result::Result::Ok(Self {
                    fid: ::core::convert::TryInto::try_into(value.fid()?)?,
                    path: ::core::convert::Into::into(value.path()?),
                    offset: ::core::convert::TryInto::try_into(value.offset()?)?,
                    stored: ::core::convert::TryInto::try_into(value.stored()?)?,
                    size: ::core::convert::TryInto::try_into(value.size()?)?,
                })
            }
        }

        impl<'a> ::planus::TableRead<'a> for EntryRef<'a> {
            #[inline]
            fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                    buffer, offset,
                )?))
            }
        }

        impl<'a> ::planus::VectorReadInner<'a> for EntryRef<'a> {
            type Error = ::planus::Error;
            const STRIDE: usize = 4;

            unsafe fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                    error_kind.with_error_location("[EntryRef]", "get", buffer.offset_from_start)
                })
            }
        }

        /// # Safety
        /// The planus compiler generates implementations that initialize
        /// the bytes in `write_values`.
        unsafe impl ::planus::VectorWrite<::planus::Offset<Entry>> for Entry {
            type Value = ::planus::Offset<Entry>;
            const STRIDE: usize = 4;
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                ::planus::WriteAs::prepare(self, builder)
            }

            #[inline]
            unsafe fn write_values(
                values: &[::planus::Offset<Entry>],
                bytes: *mut ::core::mem::MaybeUninit<u8>,
                buffer_position: u32,
            ) {
                let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                    ::planus::WriteAsPrimitive::write(
                        v,
                        ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                        buffer_position - (Self::STRIDE * i) as u32,
                    );
                }
            }
        }

        impl<'a> ::planus::ReadAsRoot<'a> for EntryRef<'a> {
            fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(
                    ::planus::SliceWithStartOffset {
                        buffer: slice,
                        offset_from_start: 0,
                    },
                    0,
                )
                .map_err(|error_kind| {
                    error_kind.with_error_location("[EntryRef]", "read_as_root", 0)
                })
            }
        }

        ///  What a container holds.
        ///
        /// Generated from these locations:
        /// * Table `Manifest` in the file `crates\engine_assets\schema\pkt.fbs:20`
        #[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
        pub struct Manifest {
            ///  The format, which only changes if old readers can't cope.
            pub format: u32,
            ///  The SHA-256 of everything after the index, in hexadecimal.
            pub sha256: ::planus::alloc::string::String,
            ///  The files.
            pub entries: ::planus::alloc::vec::Vec<self::Entry>,
        }

        #[allow(clippy::derivable_impls)]
        impl ::core::default::Default for Manifest {
            fn default() -> Self {
                Self {
                    format: 0,
                    sha256: ::core::default::Default::default(),
                    entries: ::core::default::Default::default(),
                }
            }
        }

        impl Manifest {
            /// Creates a [ManifestBuilder] for serializing an instance of this table.
            #[inline]
            pub fn builder() -> ManifestBuilder<()> {
                ManifestBuilder(())
            }

            #[allow(clippy::too_many_arguments)]
            pub fn create(
                builder: &mut ::planus::Builder,
                field_format: impl ::planus::WriteAsDefault<u32, u32>,
                field_sha256: impl ::planus::WriteAs<::planus::Offset<str>>,
                field_entries: impl ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::Entry>]>>,
            ) -> ::planus::Offset<Self> {
                let prepared_format = field_format.prepare(builder, &0);
                let prepared_sha256 = field_sha256.prepare(builder);
                let prepared_entries = field_entries.prepare(builder);

                let mut table_writer: ::planus::table_writer::TableWriter<10> =
                    ::core::default::Default::default();
                if prepared_format.is_some() {
                    table_writer.write_entry::<u32>(0);
                }
                table_writer.write_entry::<::planus::Offset<str>>(1);
                table_writer.write_entry::<::planus::Offset<[::planus::Offset<self::Entry>]>>(2);

                unsafe {
                    table_writer.finish(builder, |object_writer| {
                        if let ::core::option::Option::Some(prepared_format) = prepared_format {
                            object_writer.write::<_, _, 4>(&prepared_format);
                        }
                        object_writer.write::<_, _, 4>(&prepared_sha256);
                        object_writer.write::<_, _, 4>(&prepared_entries);
                    });
                }
                builder.current_offset()
            }
        }

        impl ::planus::WriteAs<::planus::Offset<Manifest>> for Manifest {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Manifest> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl ::planus::WriteAsOptional<::planus::Offset<Manifest>> for Manifest {
            type Prepared = ::planus::Offset<Self>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<Manifest>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl ::planus::WriteAsOffset<Manifest> for Manifest {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Manifest> {
                Manifest::create(builder, self.format, &self.sha256, &self.entries)
            }
        }

        /// Builder for serializing an instance of the [Manifest] type.
        ///
        /// Can be created using the [Manifest::builder] method.
        #[derive(Debug)]
        #[must_use]
        pub struct ManifestBuilder<State>(State);

        impl ManifestBuilder<()> {
            /// Setter for the [`format` field](Manifest#structfield.format).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn format<T0>(self, value: T0) -> ManifestBuilder<(T0,)>
            where
                T0: ::planus::WriteAsDefault<u32, u32>,
            {
                ManifestBuilder((value,))
            }

            /// Sets the [`format` field](Manifest#structfield.format) to the default value.
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn format_as_default(self) -> ManifestBuilder<(::planus::DefaultValue,)> {
                self.format(::planus::DefaultValue)
            }
        }

        impl<T0> ManifestBuilder<(T0,)> {
            /// Setter for the [`sha256` field](Manifest#structfield.sha256).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn sha256<T1>(self, value: T1) -> ManifestBuilder<(T0, T1)>
            where
                T1: ::planus::WriteAs<::planus::Offset<str>>,
            {
                let (v0,) = self.0;
                ManifestBuilder((v0, value))
            }
        }

        impl<T0, T1> ManifestBuilder<(T0, T1)> {
            /// Setter for the [`entries` field](Manifest#structfield.entries).
            #[inline]
            #[allow(clippy::type_complexity)]
            pub fn entries<T2>(self, value: T2) -> ManifestBuilder<(T0, T1, T2)>
            where
                T2: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::Entry>]>>,
            {
                let (v0, v1) = self.0;
                ManifestBuilder((v0, v1, value))
            }
        }

        impl<T0, T1, T2> ManifestBuilder<(T0, T1, T2)> {
            /// Finish writing the builder to get an [Offset](::planus::Offset) to a serialized [Manifest].
            #[inline]
            pub fn finish(self, builder: &mut ::planus::Builder) -> ::planus::Offset<Manifest>
            where
                Self: ::planus::WriteAsOffset<Manifest>,
            {
                ::planus::WriteAsOffset::prepare(&self, builder)
            }
        }

        impl<
            T0: ::planus::WriteAsDefault<u32, u32>,
            T1: ::planus::WriteAs<::planus::Offset<str>>,
            T2: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::Entry>]>>,
        > ::planus::WriteAs<::planus::Offset<Manifest>> for ManifestBuilder<(T0, T1, T2)>
        {
            type Prepared = ::planus::Offset<Manifest>;

            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Manifest> {
                ::planus::WriteAsOffset::prepare(self, builder)
            }
        }

        impl<
            T0: ::planus::WriteAsDefault<u32, u32>,
            T1: ::planus::WriteAs<::planus::Offset<str>>,
            T2: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::Entry>]>>,
        > ::planus::WriteAsOptional<::planus::Offset<Manifest>> for ManifestBuilder<(T0, T1, T2)>
        {
            type Prepared = ::planus::Offset<Manifest>;

            #[inline]
            fn prepare(
                &self,
                builder: &mut ::planus::Builder,
            ) -> ::core::option::Option<::planus::Offset<Manifest>> {
                ::core::option::Option::Some(::planus::WriteAsOffset::prepare(self, builder))
            }
        }

        impl<
            T0: ::planus::WriteAsDefault<u32, u32>,
            T1: ::planus::WriteAs<::planus::Offset<str>>,
            T2: ::planus::WriteAs<::planus::Offset<[::planus::Offset<self::Entry>]>>,
        > ::planus::WriteAsOffset<Manifest> for ManifestBuilder<(T0, T1, T2)>
        {
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> ::planus::Offset<Manifest> {
                let (v0, v1, v2) = &self.0;
                Manifest::create(builder, v0, v1, v2)
            }
        }

        /// Reference to a deserialized [Manifest].
        #[derive(Copy, Clone)]
        pub struct ManifestRef<'a>(#[allow(dead_code)] ::planus::table_reader::Table<'a>);

        impl<'a> ManifestRef<'a> {
            /// Getter for the [`format` field](Manifest#structfield.format).
            #[inline]
            pub fn format(&self) -> ::planus::Result<u32> {
                ::core::result::Result::Ok(self.0.access(0, "Manifest", "format")?.unwrap_or(0))
            }

            /// Getter for the [`sha256` field](Manifest#structfield.sha256).
            #[inline]
            pub fn sha256(&self) -> ::planus::Result<&'a ::core::primitive::str> {
                self.0.access_required(1, "Manifest", "sha256")
            }

            /// Getter for the [`entries` field](Manifest#structfield.entries).
            #[inline]
            pub fn entries(
                &self,
            ) -> ::planus::Result<::planus::Vector<'a, ::planus::Result<self::EntryRef<'a>>>>
            {
                self.0.access_required(2, "Manifest", "entries")
            }
        }

        impl<'a> ::core::fmt::Debug for ManifestRef<'a> {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                let mut f = f.debug_struct("ManifestRef");
                f.field("format", &self.format());
                f.field("sha256", &self.sha256());
                f.field("entries", &self.entries());
                f.finish()
            }
        }

        impl<'a> ::core::convert::TryFrom<ManifestRef<'a>> for Manifest {
            type Error = ::planus::Error;

            #[allow(unreachable_code)]
            fn try_from(value: ManifestRef<'a>) -> ::planus::Result<Self> {
                ::core::result::Result::Ok(Self {
                    format: ::core::convert::TryInto::try_into(value.format()?)?,
                    sha256: ::core::convert::Into::into(value.sha256()?),
                    entries: value.entries()?.to_vec_result()?,
                })
            }
        }

        impl<'a> ::planus::TableRead<'a> for ManifestRef<'a> {
            #[inline]
            fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::core::result::Result<Self, ::planus::errors::ErrorKind> {
                ::core::result::Result::Ok(Self(::planus::table_reader::Table::from_buffer(
                    buffer, offset,
                )?))
            }
        }

        impl<'a> ::planus::VectorReadInner<'a> for ManifestRef<'a> {
            type Error = ::planus::Error;
            const STRIDE: usize = 4;

            unsafe fn from_buffer(
                buffer: ::planus::SliceWithStartOffset<'a>,
                offset: usize,
            ) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(buffer, offset).map_err(|error_kind| {
                    error_kind.with_error_location("[ManifestRef]", "get", buffer.offset_from_start)
                })
            }
        }

        /// # Safety
        /// The planus compiler generates implementations that initialize
        /// the bytes in `write_values`.
        unsafe impl ::planus::VectorWrite<::planus::Offset<Manifest>> for Manifest {
            type Value = ::planus::Offset<Manifest>;
            const STRIDE: usize = 4;
            #[inline]
            fn prepare(&self, builder: &mut ::planus::Builder) -> Self::Value {
                ::planus::WriteAs::prepare(self, builder)
            }

            #[inline]
            unsafe fn write_values(
                values: &[::planus::Offset<Manifest>],
                bytes: *mut ::core::mem::MaybeUninit<u8>,
                buffer_position: u32,
            ) {
                let bytes = bytes as *mut [::core::mem::MaybeUninit<u8>; 4];
                for (i, v) in ::core::iter::Iterator::enumerate(values.iter()) {
                    ::planus::WriteAsPrimitive::write(
                        v,
                        ::planus::Cursor::new(unsafe { &mut *bytes.add(i) }),
                        buffer_position - (Self::STRIDE * i) as u32,
                    );
                }
            }
        }

        impl<'a> ::planus::ReadAsRoot<'a> for ManifestRef<'a> {
            fn read_as_root(slice: &'a [u8]) -> ::planus::Result<Self> {
                ::planus::TableRead::from_buffer(
                    ::planus::SliceWithStartOffset {
                        buffer: slice,
                        offset_from_start: 0,
                    },
                    0,
                )
                .map_err(|error_kind| {
                    error_kind.with_error_location("[ManifestRef]", "read_as_root", 0)
                })
            }
        }
    }
}
