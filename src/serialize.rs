/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

#[cfg(any(test, feature = "serde"))]
pub(crate) mod as_string_vec_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub fn serialize<S: Serializer>(
        value: &Arc<[Arc<str>]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(value.iter().map(|value| value.as_ref()))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Arc<[Arc<str>]>, D::Error> {
        Vec::<String>::deserialize(deserializer)
            .map(|values| values.into_iter().map(Arc::from).collect())
    }
}

#[cfg(feature = "rkyv")]
pub struct AsStringVec;

#[cfg(feature = "rkyv")]
const _: () = {
    use rkyv::{
        Archive, Place, Serialize,
        rancor::{Fallible, Source},
        ser::{Allocator, Writer},
        string::{ArchivedString, StringResolver},
        vec::{ArchivedVec, VecResolver},
        with::{ArchiveWith, DeserializeWith, SerializeWith},
    };
    use std::sync::Arc;

    struct StrRef<'x>(&'x str);

    impl Archive for StrRef<'_> {
        type Archived = ArchivedString;
        type Resolver = StringResolver;

        fn resolve(&self, resolver: Self::Resolver, out: Place<Self::Archived>) {
            ArchivedString::resolve_from_str(self.0, resolver, out);
        }
    }

    impl<S> Serialize<S> for StrRef<'_>
    where
        S: Fallible + Writer + ?Sized,
        S::Error: Source,
    {
        fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
            ArchivedString::serialize_from_str(self.0, serializer)
        }
    }

    impl ArchiveWith<Arc<[Arc<str>]>> for AsStringVec {
        type Archived = ArchivedVec<ArchivedString>;
        type Resolver = VecResolver;

        fn resolve_with(
            field: &Arc<[Arc<str>]>,
            resolver: Self::Resolver,
            out: Place<Self::Archived>,
        ) {
            ArchivedVec::resolve_from_len(field.len(), resolver, out);
        }
    }

    impl<S> SerializeWith<Arc<[Arc<str>]>, S> for AsStringVec
    where
        S: Fallible + Allocator + Writer + ?Sized,
        S::Error: Source,
    {
        fn serialize_with(
            field: &Arc<[Arc<str>]>,
            serializer: &mut S,
        ) -> Result<Self::Resolver, S::Error> {
            ArchivedVec::serialize_from_iter(
                field.iter().map(|value| StrRef(value.as_ref())),
                serializer,
            )
        }
    }

    impl<D> DeserializeWith<ArchivedVec<ArchivedString>, Arc<[Arc<str>]>, D> for AsStringVec
    where
        D: Fallible + ?Sized,
    {
        fn deserialize_with(
            field: &ArchivedVec<ArchivedString>,
            _: &mut D,
        ) -> Result<Arc<[Arc<str>]>, D::Error> {
            Ok(field
                .iter()
                .map(|value| Arc::from(value.as_str()))
                .collect())
        }
    }
};
