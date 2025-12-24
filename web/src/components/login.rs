use leptos::*;

#[component]
pub fn Login() -> impl IntoView {
    let (username, set_username) = create_signal(String::new());
    let (password, set_password) = create_signal(String::new());
    let (error, set_error) = create_signal(String::new());

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        // TODO: Integration with /auth/login
        set_error.set("Authentication not yet implemented".to_string());
    };

    view! {
        <div class="auth-container">
            <div class="auth-box">
                <h2>"Sign in to NGI"</h2>
                <form on:submit=on_submit>
                    <div class="input-group">
                        <label for="username">"Username"</label>
                        <input 
                            id="username"
                            type="text"
                            on:input=move |ev| set_username.set(event_target_value(&ev))
                            prop:value=username
                            placeholder="username"
                        />
                    </div>
                    <div class="input-group">
                        <label for="password">"Password"</label>
                        <input 
                            id="password"
                            type="password"
                            on:input=move |ev| set_password.set(event_target_value(&ev))
                            prop:value=password
                            placeholder="••••••••"
                        />
                    </div>
                    <button type="submit" class="btn-primary">"Sign In"</button>
                    {move || (!error.get().is_empty()).then(|| view! { 
                        <div class="error-message">{error.get()}</div>
                    })}
                </form>
            </div>
        </div>
    }
}
