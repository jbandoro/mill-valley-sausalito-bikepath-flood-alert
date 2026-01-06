use askama::Template;
use reqwest::Client;
use serde::Serialize; // Bring the trait into scope

#[derive(Template)]
#[template(path = "verification_email.html")]
pub struct VerifyTemplate<'a> {
    pub verification_link: &'a str,
}

#[derive(Debug, Serialize)]
struct NewMember {
    address: String,
    subscribed: bool,
    upsert: String,
}

impl NewMember {
    fn new(address: String) -> Self {
        NewMember {
            address,
            subscribed: true,
            upsert: "yes".to_string(),
        }
    }
}

pub struct MailgunClient {
    pub client: Client,
    pub api_key: String,
}

impl MailgunClient {
    pub async fn add_to_list(
        &self,
        list_id: &str,
        domain: &str,
        email: &str,
    ) -> reqwest::Result<()> {
        let url = format!("https://api.mailgun.net/v3/lists/{list_id}@{domain}/members");

        let new_member = NewMember::new(email.to_string());
        println!("Adding new member to mailing list: {:?}", new_member);
        self.client
            .post(url)
            .basic_auth("api", Some(&self.api_key))
            .form(&new_member)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    async fn send_list_email(
        &self,
        list_id: &str,
        subject: &str,
        body: &str,
        domain: &str,
    ) -> reqwest::Result<()> {
        let url = format!("https://api.mailgun.net/v3/{}/messages", domain);

        let params = [
            ("from", format!("Excited User <mailgun@{}>", domain)),
            ("to", list_id.to_string()),
            ("subject", subject.to_string()),
            ("text", body.to_string()),
        ];

        self.client
            .post(url)
            .basic_auth("api", Some(&self.api_key))
            .form(&params)
            .send()
            .await?;

        Ok(())
    }

    pub async fn send_verification_email(
        &self,
        to_email: &str,
        verification_link: &str,
        domain: &str,
    ) -> reqwest::Result<()> {
        let url = format!("https://api.mailgun.net/v3/{}/messages", domain);

        let subject = "Please verify your email";
        let body = format!(
            "Welcome! Please verify your email address to start receiving flood predictions for the bike path: {}",
            verification_link
        );

        let template = VerifyTemplate { verification_link };
        let html_body = template.render().unwrap_or_default();
        let params = [
            ("from", format!("No Reply <no-reply@{}>", domain)),
            ("to", to_email.to_string()),
            ("subject", subject.to_string()),
            ("text", body),
            ("html", html_body),
        ];

        self.client
            .post(url)
            .basic_auth("api", Some(&self.api_key))
            .form(&params)
            .send()
            .await?;

        Ok(())
    }
}
