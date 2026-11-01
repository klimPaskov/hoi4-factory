//! Outbound destination policy (RT-11, EXT-14).
//!
//! The agent crate makes HTTP calls to model endpoints whose base URL can come
//! from a user (a self-hosted OpenAI-compatible server). Without a policy that
//! URL is a server-side request forgery primitive. The policy refuses, by
//! default, every destination that is not on the public internet: loopback,
//! private, link-local, cloud metadata, carrier-grade NAT, unique-local,
//! multicast, documentation and benchmark ranges, and IPv6 forms that embed an
//! IPv4 address (IPv4-mapped, IPv4-compatible, NAT64, 6to4), which are judged by
//! the address they embed.
//!
//! Two layers apply. [`DestinationPolicy::check_url`] runs on the configured
//! URL and refuses bad schemes, userinfo, blocked hostnames and IP literals
//! without any DNS lookup. [`PolicyResolver`] runs at connect time inside the
//! HTTP client, resolves the name itself and refuses when ANY returned address
//! is blocked. The addresses it checked are the addresses the client connects
//! to, so a DNS answer that changes between check and use (rebinding) cannot
//! slip through. Redirects are never followed, because a redirect target would
//! be an unchecked destination.
//!
//! An operator can allow specific ranges (for example the Compose network of a
//! local model server) through [`DestinationPolicy::allowed_ranges`]. Cloud
//! metadata addresses can never be allowed.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

/// Why a destination was refused. `code` is one of `invalid_url`,
/// `scheme_not_allowed`, `userinfo_not_allowed`, `blocked_address`,
/// `blocked_hostname`, `resolution_failed`, `redirect_refused`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{code}: {detail}")]
pub struct DestinationRefusal {
    pub code: String,
    pub detail: String,
}

impl DestinationRefusal {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            detail: detail.into(),
        }
    }
}

/// The class of an IP address for policy purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressClass {
    Public,
    Loopback,
    Private,
    LinkLocal,
    /// Cloud instance metadata services. Never allowable.
    Metadata,
    Unspecified,
    Multicast,
    Broadcast,
    /// 100.64.0.0/10 shared address space.
    CarrierGradeNat,
    /// IPv6 fc00::/7.
    UniqueLocal,
    Documentation,
    Benchmark,
    /// Other reserved or special-purpose ranges (240/4, 192.0.0.0/24, Teredo, ...).
    Reserved,
}

impl AddressClass {
    pub fn is_public(self) -> bool {
        self == AddressClass::Public
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DestinationPolicy {
    /// Ranges the operator explicitly allows in addition to the public
    /// internet. Empty means the public internet only.
    #[serde(default)]
    pub allowed_ranges: Vec<IpNet>,
}

fn v4(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
    Ipv4Addr::new(a, b, c, d)
}

/// The IPv4 address an IPv6 address embeds, for the forms that tunnel or map
/// IPv4: IPv4-mapped `::ffff:a.b.c.d`, IPv4-compatible `::a.b.c.d`, NAT64
/// `64:ff9b::/96` and 6to4 `2002::/16`.
fn embedded_v4(a: &Ipv6Addr) -> Option<Ipv4Addr> {
    let o = a.octets();
    let seg = a.segments();
    if *a == Ipv6Addr::UNSPECIFIED || *a == Ipv6Addr::LOCALHOST {
        return None;
    }
    // ::ffff:a.b.c.d and ::a.b.c.d (the first 80 bits are zero)
    if seg[..5].iter().all(|s| *s == 0) && (seg[5] == 0xffff || seg[5] == 0) {
        return Some(v4(o[12], o[13], o[14], o[15]));
    }
    // 64:ff9b::/96
    if seg[0] == 0x64 && seg[1] == 0xff9b && seg[2..6].iter().all(|s| *s == 0) {
        return Some(v4(o[12], o[13], o[14], o[15]));
    }
    // 2002::/16 carries the IPv4 address in bits 16..48.
    if seg[0] == 0x2002 {
        return Some(v4(o[2], o[3], o[4], o[5]));
    }
    None
}

/// Resolve embedded-IPv4 forms to the IPv4 address they carry.
pub fn effective_ip(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(a) => embedded_v4(&a).map(IpAddr::V4).unwrap_or(ip),
        v => v,
    }
}

fn classify_v4(a: Ipv4Addr) -> AddressClass {
    let [o0, o1, o2, o3] = a.octets();
    if a == v4(169, 254, 169, 254) || a == v4(100, 100, 100, 200) || a == v4(192, 0, 0, 192) {
        return AddressClass::Metadata;
    }
    if a.is_unspecified() {
        return AddressClass::Unspecified;
    }
    if a.is_loopback() {
        return AddressClass::Loopback;
    }
    if a.is_broadcast() {
        return AddressClass::Broadcast;
    }
    if a.is_multicast() {
        return AddressClass::Multicast;
    }
    if a.is_private() {
        return AddressClass::Private;
    }
    if a.is_link_local() {
        return AddressClass::LinkLocal;
    }
    if o0 == 100 && (o1 & 0xc0) == 64 {
        return AddressClass::CarrierGradeNat;
    }
    if (o0, o1, o2) == (192, 0, 2)
        || (o0, o1, o2) == (198, 51, 100)
        || (o0, o1, o2) == (203, 0, 113)
    {
        return AddressClass::Documentation;
    }
    if o0 == 198 && (o1 & 0xfe) == 18 {
        return AddressClass::Benchmark;
    }
    // 0.0.0.0/8 "this network", 192.0.0.0/24 IETF protocol assignments,
    // 192.88.99.0/24 6to4 relay anycast, 240.0.0.0/4 reserved.
    if o0 == 0 || (o0, o1, o2) == (192, 0, 0) || (o0, o1, o2) == (192, 88, 99) || o0 >= 240 {
        return AddressClass::Reserved;
    }
    let _ = o3;
    AddressClass::Public
}

fn classify_v6(a: Ipv6Addr) -> AddressClass {
    if a == "fd00:ec2::254"
        .parse::<Ipv6Addr>()
        .unwrap_or(Ipv6Addr::UNSPECIFIED)
    {
        return AddressClass::Metadata;
    }
    if a.is_unspecified() {
        return AddressClass::Unspecified;
    }
    if a.is_loopback() {
        return AddressClass::Loopback;
    }
    if let Some(inner) = embedded_v4(&a) {
        return classify_v4(inner);
    }
    let s = a.segments();
    if a.is_multicast() {
        return AddressClass::Multicast;
    }
    if (s[0] & 0xffc0) == 0xfe80 {
        return AddressClass::LinkLocal;
    }
    // fec0::/10 site-local (deprecated) is treated as private.
    if (s[0] & 0xffc0) == 0xfec0 {
        return AddressClass::Private;
    }
    if (s[0] & 0xfe00) == 0xfc00 {
        return AddressClass::UniqueLocal;
    }
    if s[0] == 0x2001 && s[1] == 0x0db8 {
        return AddressClass::Documentation;
    }
    if s[0] == 0x3fff && (s[1] & 0xf000) == 0 {
        return AddressClass::Documentation;
    }
    if s[0] == 0x2001 && s[1] == 0x0002 && s[2] == 0 {
        return AddressClass::Benchmark;
    }
    // 100::/64 discard-only, 64:ff9b:1::/48 local-use NAT64, 2001::/32 Teredo,
    // 2001:10::/28 ORCHID and the rest of 2001::/23 IETF protocol space.
    if (s[0] == 0x0100 && s[1] == 0 && s[2] == 0 && s[3] == 0)
        || (s[0] == 0x64 && s[1] == 0xff9b && s[2] == 1)
        || (s[0] == 0x2001 && s[1] < 0x0200)
    {
        return AddressClass::Reserved;
    }
    AddressClass::Public
}

/// Classify an address. IPv6 forms that embed IPv4 are classified by the
/// embedded address.
pub fn classify(ip: IpAddr) -> AddressClass {
    match ip {
        IpAddr::V4(a) => classify_v4(a),
        IpAddr::V6(a) => classify_v6(a),
    }
}

/// Hostnames refused by name, whatever they resolve to.
fn blocked_hostname(host: &str) -> Option<&'static str> {
    let h = host.trim_end_matches('.').to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") {
        return Some("localhost names are refused");
    }
    if h == "metadata.google.internal"
        || h.ends_with(".metadata.google.internal")
        || h == "metadata"
        || h == "instance-data"
        || h == "instance-data.ec2.internal"
        || h == "metadata.azure.com"
    {
        return Some("cloud metadata names are refused");
    }
    if h.is_empty() {
        return Some("empty host");
    }
    None
}

impl DestinationPolicy {
    pub fn public_only() -> Self {
        Self::default()
    }

    pub fn allowing(ranges: &[&str]) -> Result<Self, ipnet::AddrParseError> {
        let allowed_ranges = ranges
            .iter()
            .map(|r| r.parse::<IpNet>())
            .collect::<Result<_, _>>()?;
        Ok(Self { allowed_ranges })
    }

    /// Whether the policy permits connecting to `ip`.
    pub fn permits(&self, ip: IpAddr) -> Result<(), DestinationRefusal> {
        let eff = effective_ip(ip);
        let class = classify(eff);
        if class.is_public() {
            return Ok(());
        }
        if class != AddressClass::Metadata
            && self
                .allowed_ranges
                .iter()
                .any(|r| r.contains(&eff) || r.contains(&ip))
        {
            return Ok(());
        }
        Err(DestinationRefusal::new(
            "blocked_address",
            format!("{ip} is a {class:?} address and is not in an allowed range"),
        ))
    }

    /// Validate a URL without any DNS lookup.
    pub fn check_url(&self, raw: &str) -> Result<url::Url, DestinationRefusal> {
        let u = url::Url::parse(raw)
            .map_err(|e| DestinationRefusal::new("invalid_url", e.to_string()))?;
        match u.scheme() {
            "https" | "http" => {}
            other => {
                return Err(DestinationRefusal::new(
                    "scheme_not_allowed",
                    format!("scheme {other} is not allowed"),
                ));
            }
        }
        if !u.username().is_empty() || u.password().is_some() {
            return Err(DestinationRefusal::new(
                "userinfo_not_allowed",
                "credentials in the URL are not allowed",
            ));
        }
        match u.host() {
            None => return Err(DestinationRefusal::new("invalid_url", "URL has no host")),
            Some(url::Host::Ipv4(a)) => self.permits(IpAddr::V4(a))?,
            Some(url::Host::Ipv6(a)) => self.permits(IpAddr::V6(a))?,
            Some(url::Host::Domain(d)) => {
                if let Some(why) = blocked_hostname(d) {
                    return Err(DestinationRefusal::new(
                        "blocked_hostname",
                        format!("{d}: {why}"),
                    ));
                }
            }
        }
        Ok(u)
    }
}

/// Connect-time resolver. Resolves with the system resolver and refuses when
/// any returned address is blocked (no partial filtering).
#[cfg(feature = "http")]
#[derive(Debug, Clone)]
pub struct PolicyResolver {
    policy: DestinationPolicy,
}

#[cfg(feature = "http")]
impl PolicyResolver {
    pub fn new(policy: DestinationPolicy) -> Self {
        Self { policy }
    }

    /// Resolve `host` and check every address.
    pub async fn resolve_host(
        &self,
        host: &str,
    ) -> Result<Vec<std::net::SocketAddr>, DestinationRefusal> {
        let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host((host, 0))
            .await
            .map_err(|e| DestinationRefusal::new("resolution_failed", format!("{host}: {e}")))?
            .collect();
        if addrs.is_empty() {
            return Err(DestinationRefusal::new(
                "resolution_failed",
                format!("{host}: no addresses"),
            ));
        }
        for a in &addrs {
            self.policy.permits(a.ip()).map_err(|mut r| {
                r.detail = format!("{host} resolves to {}", r.detail);
                r
            })?;
        }
        Ok(addrs)
    }
}

#[cfg(feature = "http")]
impl reqwest::dns::Resolve for PolicyResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let me = self.clone();
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addrs = me.resolve_host(&host).await?;
            let it: reqwest::dns::Addrs = Box::new(addrs.into_iter());
            Ok(it)
        })
    }
}

/// An HTTP client for model endpoints: connect-time policy resolver, no
/// redirects, no ambient proxy (a proxy would resolve names itself and bypass
/// the resolver), connect and read timeouts.
#[cfg(feature = "http")]
pub fn outbound_client(
    policy: &DestinationPolicy,
    timeout: std::time::Duration,
) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .dns_resolver(PolicyResolver::new(policy.clone()))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .connect_timeout(std::time::Duration::from_secs(10))
        .read_timeout(timeout)
        .user_agent(crate::RUNTIME_BUILD)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(s: &str) -> AddressClass {
        classify(s.parse().unwrap())
    }

    #[test]
    fn v4_classes() {
        assert_eq!(class("127.0.0.1"), AddressClass::Loopback);
        assert_eq!(class("127.9.9.9"), AddressClass::Loopback);
        for p in ["10.1.2.3", "172.16.0.1", "172.31.255.255", "192.168.1.1"] {
            assert_eq!(class(p), AddressClass::Private, "{p}");
        }
        assert_eq!(class("172.32.0.1"), AddressClass::Public);
        assert_eq!(class("169.254.1.1"), AddressClass::LinkLocal);
        assert_eq!(class("169.254.169.254"), AddressClass::Metadata);
        assert_eq!(class("100.100.100.200"), AddressClass::Metadata);
        assert_eq!(class("100.64.0.1"), AddressClass::CarrierGradeNat);
        assert_eq!(class("100.127.255.255"), AddressClass::CarrierGradeNat);
        assert_eq!(class("100.128.0.1"), AddressClass::Public);
        assert_eq!(class("0.0.0.0"), AddressClass::Unspecified);
        assert_eq!(class("224.0.0.1"), AddressClass::Multicast);
        assert_eq!(class("255.255.255.255"), AddressClass::Broadcast);
        assert_eq!(class("192.0.2.1"), AddressClass::Documentation);
        assert_eq!(class("198.51.100.7"), AddressClass::Documentation);
        assert_eq!(class("203.0.113.9"), AddressClass::Documentation);
        assert_eq!(class("198.18.0.1"), AddressClass::Benchmark);
        assert_eq!(class("198.19.255.255"), AddressClass::Benchmark);
        assert_eq!(class("240.0.0.1"), AddressClass::Reserved);
        assert_eq!(class("8.8.8.8"), AddressClass::Public);
        assert_eq!(class("1.1.1.1"), AddressClass::Public);
    }

    #[test]
    fn v6_classes() {
        assert_eq!(class("::1"), AddressClass::Loopback);
        assert_eq!(class("::"), AddressClass::Unspecified);
        assert_eq!(class("fe80::1"), AddressClass::LinkLocal);
        assert_eq!(class("fd00:ec2::254"), AddressClass::Metadata);
        assert_eq!(class("fc00::1"), AddressClass::UniqueLocal);
        assert_eq!(class("fd12:3456::1"), AddressClass::UniqueLocal);
        assert_eq!(class("ff02::1"), AddressClass::Multicast);
        assert_eq!(class("2001:db8::1"), AddressClass::Documentation);
        assert_eq!(class("2001:2::1"), AddressClass::Benchmark);
        assert_eq!(
            class("2001:0:4136:e378:8000:63bf:3fff:fdd2"),
            AddressClass::Reserved
        );
        assert_eq!(class("2606:4700:4700::1111"), AddressClass::Public);
    }

    #[test]
    fn embedded_v4_forms_are_judged_by_the_embedded_address() {
        assert_eq!(class("::ffff:127.0.0.1"), AddressClass::Loopback);
        assert_eq!(class("::ffff:10.0.0.1"), AddressClass::Private);
        assert_eq!(class("::ffff:169.254.169.254"), AddressClass::Metadata);
        assert_eq!(class("::ffff:8.8.8.8"), AddressClass::Public);
        assert_eq!(class("::127.0.0.1"), AddressClass::Loopback);
        assert_eq!(class("::10.1.1.1"), AddressClass::Private);
        assert_eq!(class("64:ff9b::7f00:1"), AddressClass::Loopback);
        assert_eq!(class("64:ff9b::a9fe:a9fe"), AddressClass::Metadata);
        assert_eq!(class("64:ff9b::c0a8:101"), AddressClass::Private);
        assert_eq!(class("64:ff9b::808:808"), AddressClass::Public);
        assert_eq!(class("2002:7f00:1::"), AddressClass::Loopback);
        assert_eq!(class("2002:0808:0808::"), AddressClass::Public);
    }

    #[test]
    fn ip_literals_are_refused_without_dns() {
        let p = DestinationPolicy::default();
        for u in [
            "http://127.0.0.1:8080/v1",
            "https://10.0.0.5/",
            "http://169.254.169.254/latest/meta-data",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[64:ff9b::7f00:1]/",
            "http://[fd00:ec2::254]/",
            "http://100.100.100.200/",
            // WHATWG parsing normalizes these to 127.0.0.1.
            "http://0x7f.1/",
            "http://2130706433/",
            "http://127.1/",
        ] {
            let r = p.check_url(u).unwrap_err();
            assert_eq!(r.code, "blocked_address", "{u}: {r}");
        }
        assert!(p.check_url("https://8.8.8.8/v1").is_ok());
        assert!(p.check_url("https://[2606:4700:4700::1111]/v1").is_ok());
    }

    #[test]
    fn schemes_userinfo_and_hostnames() {
        let p = DestinationPolicy::default();
        assert_eq!(
            p.check_url("ftp://example.com/").unwrap_err().code,
            "scheme_not_allowed"
        );
        assert_eq!(
            p.check_url("file:///etc/passwd").unwrap_err().code,
            "scheme_not_allowed"
        );
        assert_eq!(p.check_url("not a url").unwrap_err().code, "invalid_url");
        assert_eq!(
            p.check_url("https://user:pw@example.com/")
                .unwrap_err()
                .code,
            "userinfo_not_allowed"
        );
        assert_eq!(
            p.check_url("https://user@example.com/").unwrap_err().code,
            "userinfo_not_allowed"
        );
        for h in [
            "http://localhost/",
            "http://localhost./",
            "http://LOCALHOST:8080/",
            "http://api.localhost/",
            "http://metadata.google.internal/computeMetadata/v1/",
            "http://metadata/",
            "http://instance-data/",
        ] {
            assert_eq!(p.check_url(h).unwrap_err().code, "blocked_hostname", "{h}");
        }
        assert!(p.check_url("http://example.com:11434/v1").is_ok());
        assert!(p.check_url("https://api.anthropic.com").is_ok());
    }

    #[test]
    fn allowed_ranges_permit_private_but_never_metadata() {
        let p =
            DestinationPolicy::allowing(&["127.0.0.0/8", "10.0.0.0/8", "169.254.0.0/16"]).unwrap();
        assert!(p.check_url("http://127.0.0.1:9/").is_ok());
        assert!(p.check_url("http://10.1.2.3/").is_ok());
        assert!(p.check_url("http://[::ffff:10.1.2.3]/").is_ok());
        assert_eq!(
            p.check_url("http://192.168.1.1/").unwrap_err().code,
            "blocked_address"
        );
        assert_eq!(
            p.check_url("http://169.254.169.254/").unwrap_err().code,
            "blocked_address"
        );
        assert!(p.check_url("http://169.254.1.1/").is_ok());
    }

    #[test]
    fn policy_roundtrips_through_json() {
        let p = DestinationPolicy::allowing(&["127.0.0.0/8", "fd00::/8"]).unwrap();
        let back: DestinationPolicy =
            serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(p, back);
        assert_eq!(
            serde_json::from_str::<DestinationPolicy>("{}").unwrap(),
            DestinationPolicy::default()
        );
    }

    #[cfg(feature = "http")]
    #[tokio::test]
    async fn resolver_refuses_when_any_address_is_blocked() {
        let r = PolicyResolver::new(DestinationPolicy::default());
        let e = r.resolve_host("localhost").await.unwrap_err();
        assert_eq!(e.code, "blocked_address");
        let allow =
            PolicyResolver::new(DestinationPolicy::allowing(&["127.0.0.0/8", "::1/128"]).unwrap());
        assert!(allow.resolve_host("localhost").await.is_ok());
        // Only IPv4 allowed: localhost also resolves to ::1 on most hosts, and one
        // blocked address refuses the whole answer. Skip the assertion when the
        // host has no ::1 entry for localhost.
        let v4_only = PolicyResolver::new(DestinationPolicy::allowing(&["127.0.0.0/8"]).unwrap());
        let all = tokio::net::lookup_host(("localhost", 0))
            .await
            .unwrap()
            .collect::<Vec<_>>();
        if all.iter().any(|a| a.ip().is_ipv6()) {
            assert_eq!(
                v4_only.resolve_host("localhost").await.unwrap_err().code,
                "blocked_address"
            );
        }
    }
}
