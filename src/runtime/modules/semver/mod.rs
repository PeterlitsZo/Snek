use boa_engine::{
    Context, JsArgs, JsData, JsError, JsNativeError, JsResult, JsValue, NativeFunction,
    class::{Class, ClassBuilder},
    js_string,
};
use boa_gc::{Finalize, Trace};
use semver::{
    BuildMetadata as SemverBuildMetadata, Prerelease as SemverPrerelease, Version as SemverVersion,
};

#[derive(Clone, Trace, Finalize, JsData)]
struct Semver {
    // Contains no JavaScript references.
    #[unsafe_ignore_trace]
    version: SemverVersion,
}

fn error(kind: JsNativeError, message: impl Into<String>) -> JsError {
    kind.with_message(message.into()).into()
}

fn string(value: &JsValue) -> JsResult<String> {
    value
        .as_string()
        .map(|value| value.to_std_string_lossy())
        .ok_or_else(|| error(JsNativeError::typ(), "Semver arguments must be strings"))
}

fn receiver(this: &JsValue) -> JsResult<Semver> {
    let object = this
        .as_object()
        .ok_or_else(|| error(JsNativeError::typ(), "invalid Semver receiver"))?;
    let value = object
        .downcast_ref::<Semver>()
        .ok_or_else(|| error(JsNativeError::typ(), "invalid Semver receiver"))?;
    Ok(value.clone())
}

pub(in crate::runtime) fn constructor(context: &mut Context) -> JsResult<JsValue> {
    if let Some(class) = context.realm().get_class::<Semver>() {
        return Ok(class.constructor().into());
    }
    let mut builder = ClassBuilder::new::<Semver>(context);
    Semver::init(&mut builder)?;
    let class = builder.build();
    let constructor = class.constructor();
    context.realm().register_class::<Semver>(class);
    Ok(constructor.into())
}

impl Class for Semver {
    const NAME: &'static str = "Semver";
    const LENGTH: usize = 1;

    fn data_constructor(_: &JsValue, args: &[JsValue], _: &mut Context) -> JsResult<Self> {
        let input = string(args.get_or_undefined(0))?;
        let input = input.trim();
        let normalized = input
            .strip_prefix('v')
            .or_else(|| input.strip_prefix('V'))
            .filter(|rest| matches!(rest.as_bytes().first(), Some(b'0'..=b'9' | b'*')))
            .unwrap_or(input);
        let version = SemverVersion::parse(normalized)
            .map_err(|err| error(JsNativeError::error(), err.to_string()))?;
        Ok(Self { version })
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        class.method(js_string!("bump"), 1, NativeFunction::from_fn_ptr(bump));
        class.method(
            js_string!("toString"),
            0,
            NativeFunction::from_fn_ptr(to_string),
        );
        Ok(())
    }
}

fn to_string(this: &JsValue, _: &[JsValue], _: &mut Context) -> JsResult<JsValue> {
    Ok(js_string!(receiver(this)?.version.to_string()).into())
}

fn bump(this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let current = receiver(this)?;
    let op = string(args.get_or_undefined(0))?;
    let channel = args.get_or_undefined(1);
    let channel = if channel.is_undefined() {
        None
    } else {
        Some(string(channel)?)
    };
    let version = bump_version(current.version.clone(), &op, channel.as_deref())?;
    Ok(Semver::from_data(Semver { version }, context)?.into())
}

fn bump_version(
    version: SemverVersion,
    op: &str,
    channel: Option<&str>,
) -> JsResult<SemverVersion> {
    match op {
        "major" => {
            ensure_channel_unused(channel)?;
            bump_major(version)
        }
        "minor" => {
            ensure_channel_unused(channel)?;
            bump_minor(version)
        }
        "patch" => {
            ensure_channel_unused(channel)?;
            bump_patch(version)
        }
        "release" => {
            ensure_channel_unused(channel)?;
            Ok(bump_release(version))
        }
        "alpha" => {
            ensure_channel_unused(channel)?;
            bump_prerelease(version, PreChannel::Alpha)
        }
        "beta" => {
            ensure_channel_unused(channel)?;
            bump_prerelease(version, PreChannel::Beta)
        }
        "rc" => {
            ensure_channel_unused(channel)?;
            bump_prerelease(version, PreChannel::Rc)
        }
        "prepatch" => bump_prepatch(version, parse_target_channel(channel)?),
        "preminor" => bump_preminor(version, parse_target_channel(channel)?),
        "premajor" => bump_premajor(version, parse_target_channel(channel)?),
        _ => Err(error(
            JsNativeError::error(),
            "`op` must be one of `major`, `minor`, `patch`, `release`, `alpha`, `beta`, `rc`, `prepatch`, `preminor`, `premajor`",
        )),
    }
}

fn bump_major(mut version: SemverVersion) -> JsResult<SemverVersion> {
    version.major = version
        .major
        .checked_add(1)
        .ok_or_else(|| error(JsNativeError::range(), "major overflow"))?;
    version.minor = 0;
    version.patch = 0;
    version.pre = SemverPrerelease::EMPTY;
    version.build = SemverBuildMetadata::EMPTY;
    Ok(version)
}

fn bump_minor(mut version: SemverVersion) -> JsResult<SemverVersion> {
    version.minor = version
        .minor
        .checked_add(1)
        .ok_or_else(|| error(JsNativeError::range(), "minor overflow"))?;
    version.patch = 0;
    version.pre = SemverPrerelease::EMPTY;
    version.build = SemverBuildMetadata::EMPTY;
    Ok(version)
}

fn bump_patch(mut version: SemverVersion) -> JsResult<SemverVersion> {
    version.patch = version
        .patch
        .checked_add(1)
        .ok_or_else(|| error(JsNativeError::range(), "patch overflow"))?;
    version.pre = SemverPrerelease::EMPTY;
    version.build = SemverBuildMetadata::EMPTY;
    Ok(version)
}

fn bump_release(mut version: SemverVersion) -> SemverVersion {
    version.pre = SemverPrerelease::EMPTY;
    version.build = SemverBuildMetadata::EMPTY;
    version
}

fn bump_prepatch(version: SemverVersion, target: PreChannel) -> JsResult<SemverVersion> {
    let version = bump_patch(version)?;
    start_prerelease(version, target)
}

fn bump_preminor(version: SemverVersion, target: PreChannel) -> JsResult<SemverVersion> {
    let version = bump_minor(version)?;
    start_prerelease(version, target)
}

fn bump_premajor(version: SemverVersion, target: PreChannel) -> JsResult<SemverVersion> {
    let version = bump_major(version)?;
    start_prerelease(version, target)
}

fn bump_prerelease(version: SemverVersion, target: PreChannel) -> JsResult<SemverVersion> {
    let mut version = version;
    version.build = SemverBuildMetadata::EMPTY;

    let pre = version.pre.as_str();
    if pre.is_empty() {
        version.patch = version
            .patch
            .checked_add(1)
            .ok_or_else(|| error(JsNativeError::range(), "patch overflow"))?;
        version.pre = parse_prerelease(&format!("{}.1", target.as_str()))?;
        return Ok(version);
    }

    let (current_channel, current_number) = parse_channel_and_number(pre)?;
    if current_channel.rank() > target.rank() {
        return Err(error(
            JsNativeError::error(),
            format!(
                "does not allow prerelease downgrade from `{}` to `{}`",
                current_channel.as_str(),
                target.as_str()
            ),
        ));
    }

    let next_number = if current_channel.rank() < target.rank() {
        1
    } else {
        current_number.unwrap_or(0).saturating_add(1)
    };

    version.pre = parse_prerelease(&format!("{}.{}", target.as_str(), next_number))?;
    Ok(version)
}

fn start_prerelease(mut version: SemverVersion, target: PreChannel) -> JsResult<SemverVersion> {
    version.pre = parse_prerelease(&format!("{}.1", target.as_str()))?;
    version.build = SemverBuildMetadata::EMPTY;
    Ok(version)
}

fn ensure_channel_unused(channel: Option<&str>) -> JsResult<()> {
    if channel.is_some() {
        return Err(error(
            JsNativeError::error(),
            "`channel` is only supported for `prepatch`, `preminor`, and `premajor`",
        ));
    }
    Ok(())
}

fn parse_target_channel(channel: Option<&str>) -> JsResult<PreChannel> {
    let value = channel.unwrap_or("alpha");
    PreChannel::parse(value).ok_or_else(|| {
        error(
            JsNativeError::error(),
            format!("`channel` must be one of `alpha`, `beta`, `rc`, got `{value}`"),
        )
    })
}

fn parse_prerelease(value: &str) -> JsResult<SemverPrerelease> {
    SemverPrerelease::new(value).map_err(|err| {
        error(
            JsNativeError::error(),
            format!("invalid prerelease value `{value}`: {err}"),
        )
    })
}

fn parse_channel_and_number(pre: &str) -> JsResult<(PreChannel, Option<u64>)> {
    let parts: Vec<&str> = pre.split('.').collect();
    let channel =
        PreChannel::parse(parts.first().copied().unwrap_or_default()).ok_or_else(|| {
            error(
                JsNativeError::error(),
                format!("only supports prerelease channels `alpha`, `beta`, `rc`, got `{pre}`"),
            )
        })?;

    if parts.len() == 1 {
        return Ok((channel, None));
    }
    if parts.len() > 2 {
        return Err(error(
            JsNativeError::error(),
            format!(
                "prerelease `{pre}` is unsupported; expected `<channel>` or `<channel>.<number>`"
            ),
        ));
    }

    let number = parts[1].parse::<u64>().map_err(|_| {
        error(
            JsNativeError::error(),
            format!("prerelease `{pre}` is unsupported; expected numeric suffix"),
        )
    })?;
    Ok((channel, Some(number)))
}

#[derive(Clone, Copy)]
enum PreChannel {
    Alpha,
    Beta,
    Rc,
}

impl PreChannel {
    fn parse(input: &str) -> Option<Self> {
        match input {
            "alpha" => Some(Self::Alpha),
            "beta" => Some(Self::Beta),
            "rc" => Some(Self::Rc),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Alpha => "alpha",
            Self::Beta => "beta",
            Self::Rc => "rc",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Alpha => 1,
            Self::Beta => 2,
            Self::Rc => 3,
        }
    }
}
