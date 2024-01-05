use poise::serenity_prelude::{CreateEmbed, Member};
use poise::CreateReply;
use sqlx::types::chrono::Utc;

use crate::checks;
use crate::Context;
use crate::Error;

#[poise::command(
    prefix_command,
    slash_command,
    check = "checks::is_onboardable",
    check = "checks::setup_onboarding"
)]
pub async fn deny(ctx: Context<'_>, member: Member, reason: String) -> Result<(), Error> {
    let data = ctx.data();

    let Some(onboarding_id) = crate::setup::get_onboarding_id(&ctx).await? else {
        return Err("Onboarding ID not found for this server?".into());
    };

    let onboard_state = sqlx::query!(
        "SELECT state FROM staff_onboardings WHERE user_id = $1 AND id = $2",
        ctx.author().id.to_string(),
        onboarding_id
    )
    .fetch_one(&data.pool)
    .await?
    .state
    .parse::<crate::states::OnboardState>()?;

    match onboard_state {
        crate::states::OnboardState::Pending => {
            Err(format!("Please run ``{}queue`` to get started!", ctx.prefix()).into())
        }
        crate::states::OnboardState::Claimed => {
            if member.user.id != crate::config::CONFIG.test_bot {
                ctx.send(
                    CreateReply::default().embed(
                        CreateEmbed::default()
                            .title("Invalid Bot")
                            .description("You can only deny the test bot!")
                            .color(0xFF0000),
                    ),
                )
                .await?;

                return Ok(());
            }

            if reason.len() < 30 {
                ctx.send(
                    CreateReply::default().embed(
                        CreateEmbed::default()
                            .title("Invalid Reason")
                            .description(
                                "Please provide a reason that is at least 30 characters long!",
                            )
                            .color(0xFF0000),
                    ),
                )
                .await?;

                return Ok(());
            }

            sqlx::query!(
                "UPDATE staff_onboardings SET state = $1, verdict = $2 WHERE user_id = $3 AND id = $4",
                crate::states::OnboardState::InQuiz.to_string(),
                serde_json::json!({
                    "action": "deny",
                    "reason": reason,
                    "end_review_time": Utc::now().timestamp(), // Current time review ended
                }),
                ctx.author().id.to_string(),
                onboarding_id
            )
            .execute(&data.pool)
            .await?;

            // Try kicking the test bot from the server now
            ctx.guild_id()
                .ok_or("Failed to get guild")?
                .kick_with_reason(
                    &ctx.serenity_context(),
                    crate::config::CONFIG.test_bot,
                    "Activated Paradise Protection Protocol",
                )
                .await?;

            ctx.say("Oh great work in denying this bo-!").await?;

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            ctx.say(format!(
                "
*Paradise Protection Protocol activated, deploying defenses!!!*

Oh well, good luck with the quiz: {}/onboarding/quiz/{}
                ",
                crate::config::CONFIG.panel_url,
                onboarding_id
            ))
            .await?;

            Ok(())
        }
        crate::states::OnboardState::InQuiz => Err(format!(
            "
*Paradise Protection Protocol activated*

Visit {}/onboarding/quiz/{} to take the quiz!
                ",
            crate::config::CONFIG.panel_url,
            onboarding_id
        )
        .into()),
        _ => Err("Hmm... seems like you can't use this command yet!".into()), // TODO, remove
    }
}
