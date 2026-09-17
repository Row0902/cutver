use semver::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bump {
    Patch,
    Minor,
    Major,
}

pub fn bump(version: &Version, bump: Bump) -> Version {
    let mut next = Version::new(version.major, version.minor, version.patch);
    match bump {
        Bump::Patch => next.patch += 1,
        Bump::Minor => {
            next.minor += 1;
            next.patch = 0;
        }
        Bump::Major => {
            next.major += 1;
            next.minor = 0;
            next.patch = 0;
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        s.parse().unwrap()
    }

    #[test]
    fn patch_bump() {
        assert_eq!(bump(&v("1.2.3"), Bump::Patch), v("1.2.4"));
    }

    #[test]
    fn minor_bump() {
        assert_eq!(bump(&v("1.2.3"), Bump::Minor), v("1.3.0"));
    }

    #[test]
    fn major_bump() {
        assert_eq!(bump(&v("1.2.3"), Bump::Major), v("2.0.0"));
    }

    #[test]
    fn drops_prerelease_and_build_metadata() {
        assert_eq!(bump(&v("1.2.3-alpha.1"), Bump::Minor), v("1.3.0"));
        assert_eq!(bump(&v("1.2.3+build.42"), Bump::Patch), v("1.2.4"));
        assert_eq!(bump(&v("1.2.3-rc.1+build.2"), Bump::Major), v("2.0.0"));
    }

    #[test]
    fn zero_components_stay_zero() {
        assert_eq!(bump(&v("0.5.1"), Bump::Minor), v("0.6.0"));
    }

    #[test]
    fn major_bump_from_zero_zero_x() {
        assert_eq!(bump(&v("0.0.7"), Bump::Major), v("1.0.0"));
    }

    #[test]
    fn patch_and_minor_from_zero_zero_x() {
        assert_eq!(bump(&v("0.0.7"), Bump::Patch), v("0.0.8"));
        assert_eq!(bump(&v("0.0.7"), Bump::Minor), v("0.1.0"));
    }

    #[test]
    fn field_math_idempotency() {
        let base = v("1.2.3");
        assert_eq!(bump(&bump(&base, Bump::Patch), Bump::Patch), v("1.2.5"));
        assert_eq!(bump(&bump(&base, Bump::Minor), Bump::Minor), v("1.4.0"));
        assert_eq!(bump(&bump(&base, Bump::Major), Bump::Major), v("3.0.0"));
    }
}
