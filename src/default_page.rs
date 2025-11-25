
pub fn default_index() -> &'static str {
    r#"

 🌐 Site accessed and served through Nym mixnet

This website is hosted on a server and both accessed by visitors and served to them through the **Nym mixnet**, a decentralized network that anonymizes connections for enhanced privacy.

---

### How it works

Your traffic is routed through Nym's **five-hop mix network**:  

1. Entry gateway  
2. Three mix nodes  
3. Exit gateway  

Using **Sphinx packet encryption**, packet shuffling, and cover traffic, Nym hides your **IP address**, **location**, and **browsing patterns**, protecting both **data and metadata** from surveillance.  

Unlike Tor, which uses three hops and supports hidden services, or VPNs, which are centralized and may expose metadata, **Nym provides stronger privacy** for standard internet access.

---

### Current status

No content has been added to this site yet.  

To learn more about Nym's privacy technology, visit [nym.com](https://nym.com) or contact the site administrator.
"#
}



pub fn default_404() -> &'static str {
    r#"
# ❌ 404 — Page Not Found

The page you’re looking for doesn’t exist.

This site is accessed and served through the **Nym mixnet**, which anonymizes both visitors and the server by routing traffic through a decentralized, privacy-preserving network.

---

### Why you’re seeing this page

- The content may not exist yet  
- The site administrator may have removed or renamed the resource  

---

### About Nym privacy routing

Your traffic is routed through Nym’s **five-hop mixnet**, using layered Sphinx packet encryption and cover traffic to hide:

- IP addresses  
- Metadata  
- Network patterns  

This protects both users and servers from surveillance and traffic analysis.

---

If you believe this is an error, please contact the site administrator or return to the home page.
"#
}