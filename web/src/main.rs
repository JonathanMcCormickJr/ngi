mod api;
mod components;
use components::login::Login;
use components::ticket_list::TicketList;
use leptos::*;
use leptos_router::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(|| view! { <App/> })
}

#[component]
fn App() -> impl IntoView {
    view! {
        <Router>
            <main>
                <Routes>
                    <Route path="/" view=|| view! { <Login/> }/>
                    <Route path="/tickets" view=|| view! { <TicketList/> }/>
                </Routes>
            </main>
        </Router>
    }
}
