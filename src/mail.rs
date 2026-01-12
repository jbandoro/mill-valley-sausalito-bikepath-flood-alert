use crate::models::{FloodDisplay, User};
use askama::Template;
use lettre::message::MultiPart;
use thiserror::Error;

use lettre::message::header::{HeaderName, HeaderValue};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

use lettre::transport::smtp::client::{Tls, TlsParameters};

#[derive(Template)]
#[template(path = "verification_email.html")]
pub struct VerifyTemplate<'a> {
    pub verification_link: &'a str,
    pub unsubscribe_link: &'a str,
}

#[derive(Template)]
#[template(path = "notification_email.html")]
pub struct NotificationTemplate<'a> {
    pub predictions: &'a Vec<FloodDisplay>,
    pub homepage_url: &'a str,
    pub unsubscribe_link: &'a str,
}

#[derive(Error, Debug)]
pub enum EmailError {
    #[error("Email address parsing error: {0}")]
    InvalidEmailAddress(#[from] lettre::address::AddressError),
    #[error("Message build error: {0}")]
    MessageBuildError(#[from] lettre::error::Error),
    #[error("SMTP transport error: {0}")]
    SmtpTransportError(#[from] lettre::transport::smtp::Error),
}

pub struct SmtpClient {
    pub transport: AsyncSmtpTransport<Tokio1Executor>,
    pub from_email: String,
    pub base_url: String,
}

impl SmtpClient {
    pub fn new(
        host: String,
        port: u16,
        user: String,
        pass: String,
        from_email: String,
        base_url: String,
    ) -> Self {
        let creds = Credentials::new(user, pass);

        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(&host[..])
            .expect("Failed to create SMTP transport")
            .port(port)
            .credentials(creds)
            .tls(Tls::Required(
                TlsParameters::new(host.clone()).expect("Failed to create TLS parameters"),
            ))
            .build();

        Self {
            transport,
            from_email,
            base_url,
        }
    }

    pub async fn send_verification_email(
        &self,
        user: &User,
        verification_link: &str,
        unsubscribe_link: &str,
    ) -> Result<(), EmailError> {
        let subject = "Please verify your email";

        let template = VerifyTemplate {
            verification_link,
            unsubscribe_link,
        };
        let html_body = template.render().unwrap_or_default();
        let text_body = format!(
            "Welcome! Please verify your email address: {}",
            verification_link
        );
        let email = self.build_email(subject, &text_body, &html_body, user, unsubscribe_link)?;
        self.transport.send(email).await?;
        Ok(())
    }

    pub async fn send_list_notification_email(
        &self,
        predictions: Vec<FloodDisplay>,
        recipients: Vec<User>,
        unsubscribe_links: Vec<String>,
    ) -> Result<(), EmailError> {
        let subject = "MV-Sausalito Bike Path Flooding Forecasted";

        for (user, unsubscribe_link) in recipients.iter().zip(unsubscribe_links.iter()) {
            let template = NotificationTemplate {
                predictions: &predictions,
                homepage_url: &self.base_url,
                unsubscribe_link,
            };
            let html_body = template.render().unwrap_or_default();
            let text_body = format!(
                "Upcoming potential floods for the MV-Sausalito bike path. Please visit {} for details.\n\nUnsubscribe link: {}",
                &self.base_url, &unsubscribe_link
            );

            let email_msg =
                self.build_email(subject, &text_body, &html_body, user, unsubscribe_link)?;

            self.transport.send(email_msg).await?;
        }

        Ok(())
    }

    pub fn build_email(
        &self,
        subject: &str,
        text_body: &str,
        html_body: &str,
        user: &User,
        unsubscribe_link: &str,
    ) -> Result<Message, EmailError> {
        Ok(Message::builder()
            .from(self.from_email.parse()?)
            .to(user.email.parse()?)
            .subject(subject)
            .raw_header(HeaderValue::new(
                HeaderName::new_from_ascii_str("List-Unsubscribe"),
                format!("<{}>", unsubscribe_link),
            ))
            .raw_header(HeaderValue::new(
                HeaderName::new_from_ascii_str("List-Unsubscribe-Post"),
                "List-Unsubscribe=One-Click".to_string(),
            ))
            .multipart(
                MultiPart::alternative()
                    .singlepart(lettre::message::SinglePart::plain(format!(
                        "{}\n\nUnsubscribe link:{}",
                        text_body, unsubscribe_link
                    )))
                    .singlepart(lettre::message::SinglePart::html(html_body.to_string())),
            )?)
    }
}
