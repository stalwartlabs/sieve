use crate::{
    compiler::grammar::{
        tests::test_mailbox::{TestMetadata, TestMetadataExists},
        MatchType,
    },
    Context, Metadata,
};

impl TestMetadata {
    pub(crate) fn exec(&self, ctx: &mut Context) -> bool {
        let metadata = match &self.medatata {
            Metadata::Server { annotation } => Metadata::Server {
                annotation: ctx.eval_string(annotation),
            },
            Metadata::Mailbox { name, annotation } => Metadata::Mailbox {
                name: ctx.eval_string(name),
                annotation: ctx.eval_string(annotation),
            },
        };

        let value = if let Some((_, value)) = [&ctx.metadata, &ctx.runtime.metadata]
            .into_iter()
            .flatten()
            .find(|(m, _)| match (m, &metadata) {
                (Metadata::Server { annotation: a }, Metadata::Server { annotation: b }) => {
                    a.eq_ignore_ascii_case(b)
                }
                (
                    Metadata::Mailbox {
                        name: a,
                        annotation: c,
                    },
                    Metadata::Mailbox {
                        name: b,
                        annotation: d,
                    },
                ) => a.eq(b) && c.eq_ignore_ascii_case(d),
                _ => false,
            }) {
            value.as_ref()
        } else {
            return false ^ self.is_not;
        };

        let mut result = false;
        if let MatchType::Count(match_type) = &self.match_type {
            for key in &self.key_list {
                if match_type.cmp_num(1.0, ctx.eval_string(key).as_ref()) {
                    result = true;
                    break;
                }
            }
        } else {
            let mut captured_values = Vec::new();

            for key in &self.key_list {
                let key = ctx.eval_string(key);
                result = match &self.match_type {
                    MatchType::Is => self.comparator.is(value, key.as_ref()),
                    MatchType::Contains => self.comparator.contains(value, key.as_ref()),
                    MatchType::Value(relation) => {
                        self.comparator.relational(relation, value, key.as_ref())
                    }
                    MatchType::Matches(capture_positions) => self.comparator.matches(
                        value,
                        key.as_ref(),
                        *capture_positions,
                        &mut captured_values,
                    ),
                    MatchType::Regex(capture_positions) => self.comparator.regex(
                        value,
                        key.as_ref(),
                        *capture_positions,
                        &mut captured_values,
                    ),
                    _ => false,
                };

                if result {
                    break;
                }
            }

            if !captured_values.is_empty() {
                ctx.set_match_variables(captured_values);
            }
        }

        result ^ self.is_not
    }
}

impl TestMetadataExists {
    pub(crate) fn exec(&self, ctx: &Context) -> bool {
        let mailbox = self
            .mailbox
            .as_ref()
            .map(|s| ctx.eval_string(s).into_owned());
        let mut annotations = ctx.eval_strings(&self.annotation_names);

        for (metadata, _) in [&ctx.metadata, &ctx.runtime.metadata].into_iter().flatten() {
            match (metadata, mailbox.as_ref()) {
                (Metadata::Server { annotation }, None) => {
                    annotations.retain(|a| !a.eq_ignore_ascii_case(annotation))
                }
                (Metadata::Mailbox { name, annotation }, Some(mailbox)) if name.eq(mailbox) => {
                    annotations.retain(|a| !a.eq_ignore_ascii_case(annotation));
                }
                _ => (),
            }
            if annotations.is_empty() {
                return true ^ self.is_not;
            }
        }

        false ^ self.is_not
    }
}