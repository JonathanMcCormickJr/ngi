use crate::api::{self, CreateTicketRequest, CreateUserRequest, Ticket, UpdateTicketRequest};
use leptos::*;
use wasm_bindgen_futures::spawn_local;

#[component]
pub fn TicketList() -> impl IntoView {
    let token = api::get_token();

    let (ticket_id_input, set_ticket_id_input) = create_signal(String::new());
    let (ticket, set_ticket) = create_signal::<Option<Ticket>>(None);
    let (message, set_message) = create_signal(String::new());
    let (error, set_error) = create_signal(String::new());

    let (new_title, set_new_title) = create_signal(String::new());
    let (new_project, set_new_project) = create_signal(String::new());
    let (new_account_uuid, set_new_account_uuid) = create_signal(String::new());
    let (new_priority, set_new_priority) = create_signal("1".to_string());

    let (update_title, set_update_title) = create_signal(String::new());
    let (update_project, set_update_project) = create_signal(String::new());
    let (update_priority, set_update_priority) = create_signal(String::new());
    let (update_status, set_update_status) = create_signal(String::new());

    let (create_user_name, set_create_user_name) = create_signal(String::new());
    let (create_user_password, set_create_user_password) = create_signal(String::new());
    let (create_user_email, set_create_user_email) = create_signal(String::new());
    let (create_user_display_name, set_create_user_display_name) = create_signal(String::new());
    let (create_user_role, set_create_user_role) = create_signal("2".to_string());

    let on_lookup = {
        let token = token.clone();
        move |_| {
            let Some(token) = token.clone() else {
                set_error.set("Missing auth token. Please sign in again.".to_string());
                return;
            };

            let ticket_id = match ticket_id_input.get().parse::<u64>() {
                Ok(id) => id,
                Err(_) => {
                    set_error.set("Ticket number must be a valid integer.".to_string());
                    return;
                }
            };

            set_error.set(String::new());
            set_message.set(String::new());

            spawn_local(async move {
                match api::fetch_ticket(&token, ticket_id).await {
                    Ok(found) => {
                        set_update_title.set(found.title.clone());
                        set_update_project.set(found.project.clone());
                        set_update_priority.set(found.priority.to_string());
                        set_update_status.set(found.status.to_string());
                        set_ticket.set(Some(found));
                    }
                    Err(err) => set_error.set(err),
                }
            });
        }
    };

    let on_create_ticket = {
        let token = token.clone();
        move |_| {
            let Some(token) = token.clone() else {
                set_error.set("Missing auth token. Please sign in again.".to_string());
                return;
            };

            let priority = match new_priority.get().parse::<i32>() {
                Ok(value) => value,
                Err(_) => {
                    set_error.set("Priority must be a valid integer enum value.".to_string());
                    return;
                }
            };

            let payload = CreateTicketRequest {
                title: new_title.get(),
                project: new_project.get(),
                account_uuid: new_account_uuid.get(),
                symptom: 1,
                priority,
            };

            set_error.set(String::new());
            set_message.set(String::new());

            spawn_local(async move {
                match api::create_ticket(&token, &payload).await {
                    Ok(created) => {
                        set_ticket_id_input.set(created.ticket_id.to_string());
                        set_update_title.set(created.title.clone());
                        set_update_project.set(created.project.clone());
                        set_update_priority.set(created.priority.to_string());
                        set_update_status.set(created.status.to_string());
                        set_ticket.set(Some(created.clone()));
                        set_message.set(format!("Ticket #{} created.", created.ticket_id));
                    }
                    Err(err) => set_error.set(err),
                }
            });
        }
    };

    let on_update_ticket = {
        let token = token.clone();
        move |_| {
            let Some(token) = token.clone() else {
                set_error.set("Missing auth token. Please sign in again.".to_string());
                return;
            };

            let ticket_id = match ticket_id_input.get().parse::<u64>() {
                Ok(id) => id,
                Err(_) => {
                    set_error.set("Ticket number must be a valid integer.".to_string());
                    return;
                }
            };

            let priority = if update_priority.get().trim().is_empty() {
                None
            } else {
                match update_priority.get().parse::<i32>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        set_error
                            .set("Update priority must be a valid integer enum value.".to_string());
                        return;
                    }
                }
            };

            let status = if update_status.get().trim().is_empty() {
                None
            } else {
                match update_status.get().parse::<i32>() {
                    Ok(value) => Some(value),
                    Err(_) => {
                        set_error
                            .set("Update status must be a valid integer enum value.".to_string());
                        return;
                    }
                }
            };

            let payload = UpdateTicketRequest {
                title: (!update_title.get().trim().is_empty()).then(|| update_title.get()),
                project: (!update_project.get().trim().is_empty()).then(|| update_project.get()),
                priority,
                status,
            };

            set_error.set(String::new());
            set_message.set(String::new());

            spawn_local(async move {
                match api::update_ticket(&token, ticket_id, &payload).await {
                    Ok(updated) => {
                        set_ticket.set(Some(updated.clone()));
                        set_message.set(format!("Ticket #{} updated.", updated.ticket_id));
                    }
                    Err(err) => set_error.set(err),
                }
            });
        }
    };

    let on_create_user = {
        let token = token.clone();
        move |_| {
            let Some(token) = token.clone() else {
                set_error.set("Missing auth token. Please sign in again.".to_string());
                return;
            };

            let role = match create_user_role.get().parse::<i32>() {
                Ok(value) => value,
                Err(_) => {
                    set_error.set("Role must be a valid integer enum value.".to_string());
                    return;
                }
            };

            let payload = CreateUserRequest {
                username: create_user_name.get(),
                password: create_user_password.get(),
                email: create_user_email.get(),
                display_name: create_user_display_name.get(),
                role,
            };

            set_error.set(String::new());
            set_message.set(String::new());

            spawn_local(async move {
                match api::create_user(&token, &payload).await {
                    Ok(()) => set_message.set("User created successfully.".to_string()),
                    Err(err) => set_error.set(err),
                }
            });
        }
    };

    view! {
        <div class="dashboard-container">
            <header class="dashboard-header">
                <h2>"MVP Demo Console"</h2>
            </header>

            {move || token.is_none().then(|| view! {
                <div class="error-message">"No token found. Sign in on / first."</div>
            })}

            <div class="ticket-list">
                <h3>"1) Create User"</h3>
                <div class="input-group">
                    <label for="new-user-username">"Username"</label>
                    <input id="new-user-username" type="text" on:input=move |ev| set_create_user_name.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="new-user-password">"Password"</label>
                    <input id="new-user-password" type="password" on:input=move |ev| set_create_user_password.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="new-user-email">"Email"</label>
                    <input id="new-user-email" type="text" on:input=move |ev| set_create_user_email.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="new-user-display-name">"Display Name"</label>
                    <input id="new-user-display-name" type="text" on:input=move |ev| set_create_user_display_name.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="new-user-role">"Role Enum"</label>
                    <input id="new-user-role" type="text" prop:value=create_user_role on:input=move |ev| set_create_user_role.set(event_target_value(&ev)) />
                </div>
                <button class="btn-primary" on:click=on_create_user>"Create User"</button>

                <h3 style="margin-top: 1.5rem;">"2) Create Ticket"</h3>
                <div class="input-group">
                    <label for="new-ticket-title">"Title"</label>
                    <input id="new-ticket-title" type="text" on:input=move |ev| set_new_title.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="new-ticket-project">"Project"</label>
                    <input id="new-ticket-project" type="text" on:input=move |ev| set_new_project.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="new-ticket-account-uuid">"Account UUID"</label>
                    <input id="new-ticket-account-uuid" type="text" on:input=move |ev| set_new_account_uuid.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="new-ticket-priority">"Priority Enum"</label>
                    <input id="new-ticket-priority" type="text" prop:value=new_priority on:input=move |ev| set_new_priority.set(event_target_value(&ev)) />
                </div>
                <button class="btn-primary" on:click=on_create_ticket>"Create Ticket"</button>

                <h3 style="margin-top: 1.5rem;">"3) Lookup Ticket By Number"</h3>
                <div class="input-group">
                    <label for="ticket-id">"Ticket #"</label>
                    <input
                        id="ticket-id"
                        type="text"
                        prop:value=ticket_id_input
                        on:input=move |ev| set_ticket_id_input.set(event_target_value(&ev))
                    />
                </div>
                <button class="btn-primary" on:click=on_lookup>"Load Ticket"</button>

                <h3 style="margin-top: 1.5rem;">"4) Update Ticket"</h3>
                <div class="input-group">
                    <label for="update-title">"Title"</label>
                    <input id="update-title" type="text" prop:value=update_title on:input=move |ev| set_update_title.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="update-project">"Project"</label>
                    <input id="update-project" type="text" prop:value=update_project on:input=move |ev| set_update_project.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="update-priority">"Priority Enum"</label>
                    <input id="update-priority" type="text" prop:value=update_priority on:input=move |ev| set_update_priority.set(event_target_value(&ev)) />
                </div>
                <div class="input-group">
                    <label for="update-status">"Status Enum"</label>
                    <input id="update-status" type="text" prop:value=update_status on:input=move |ev| set_update_status.set(event_target_value(&ev)) />
                </div>
                <button class="btn-primary" on:click=on_update_ticket>"Update Loaded Ticket"</button>

                {move || (!message.get().is_empty()).then(|| view! {
                    <div class="loading">{message.get()}</div>
                })}
                {move || (!error.get().is_empty()).then(|| view! {
                    <div class="error-message">{error.get()}</div>
                })}

                {move || ticket.get().map(|t| view! {
                    <div style="margin-top: 1.5rem; padding: 1rem; border: 1px solid #30363d; border-radius: 8px;">
                        <h4>{format!("Loaded Ticket #{}", t.ticket_id)}</h4>
                        <p>{format!("Title: {}", t.title)}</p>
                        <p>{format!("Project: {}", t.project)}</p>
                        <p>{format!("Priority(enum): {}", t.priority)}</p>
                        <p>{format!("Status(enum): {}", t.status)}</p>
                    </div>
                })}
            </div>
        </div>
    }
}
