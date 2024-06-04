use std::borrow::Cow;

pub struct Config {
    pub(crate) filter_allow: Cow<'static, [Cow<'static, str>]>,
    pub(crate) filter_ignore: Cow<'static, [Cow<'static, str>]>,
}

pub struct Builder {
    filter_allow: Cow<'static, [Cow<'static, str>]>,
    filter_ignore: Cow<'static, [Cow<'static, str>]>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    pub fn new() -> Self {
        Self {
            filter_allow: Cow::Borrowed(&[]),
            filter_ignore: Cow::Borrowed(&[]),
        }
    }
    pub fn allow(mut self, filter: &'static str) -> Self {
        let mut list = self.filter_allow.to_vec();
        list.push(Cow::Borrowed(filter));
        self.filter_allow = Cow::Owned(list);
        self
    }

    pub fn ignore(mut self, filter: &'static str) -> Self {
        let mut list = self.filter_ignore.to_vec();
        list.push(Cow::Borrowed(filter));
        self.filter_ignore = Cow::Owned(list);
        self
    }

    pub fn build(self) -> Config {
        Config {
            filter_allow: self.filter_allow,
            filter_ignore: self.filter_ignore,
        }
    }
}
