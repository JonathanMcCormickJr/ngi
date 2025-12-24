use leptos::*;
use crate::api::{fetch_tickets, Ticket};

#[component]
pub fn TicketList() -> impl IntoView {
    let tickets = create_resource(|| (), |_| async move { fetch_tickets().await });

    view! {
        <div class="dashboard-container">
            <header class="dashboard-header">
                <h2>"My Tickets"</h2>
                <button class="btn-primary">"New Ticket"</button>
            </header>
            
            <div class="ticket-list">
                {move || match tickets.get() {
                    None => view! { <div class="loading">"Loading tickets..."</div> }.into_view(),
                    Some(Ok(data)) => view! {
                        <table class="ticket-table">
                            <thead>
                                <tr>
                                    <th>"ID"</th>
                                    <th>"Title"</th>
                                    <th>"Priority"</th>
                                    <th>"Status"</th>
                                    <th>"Actions"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For
                                    each=move || data.clone()
                                    key=|ticket| ticket.id
                                    children=move |ticket| view! {
                                        <tr>
                                            <td>{"#"}{ticket.id}</td>
                                            <td class="ticket-title">{ticket.title}</td>
                                            <td><span class="badge">{ticket.priority}</span></td>
                                            <td><span class="badge">{ticket.status}</span></td>
                                            <td><button class="btn-sm">"View"</button></td>
                                        </tr>
                                    }
                                />
                            </tbody>
                        </table>
                    }.into_view(),
                    Some(Err(e)) => view! { <div class="error-message">{e}</div> }.into_view(),
                }}
            </div>
        </div>
    }
}
