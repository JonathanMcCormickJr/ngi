# Feature Requests

This document is intended to track user-submitted feature requests for the InfoVulcan project. If you have a feature you'd like to see implemented, please submit a pull request adding your request to this file, including a brief description and any relevant details.

NOT ALL REQUESTS WILL BE IMPLEMENTED. Each request will be evaluated based on its feasibility, alignment with project goals, and overall benefit to users.

## Feature Requests
Core product capabilities:
- **Advanced Search and Filtering** - InfoVulcan must provide powerful search and filtering options, allowing users to quickly locate tickets based on various criteria such as status, priority, assignee, tags, and custom fields.
- **Broader Organizational Use** - InfoVulcan must be adaptable for use beyond just the company's Tech Support Department, allowing other departments to utilize the system for their ticketing needs.
- **Extensibility** - InfoVulcan must be easily extensible to accommodate future requirements and integrations. Enums should be designed to allow new variants without breaking existing data.
- **Faster** - InfoVulcan must be faster than the company's existing ticketing system.
- **Highly Responsive** - InfoVulcan must provide a highly responsive user experience, with minimal latency for all operations, even under heavy load. The GUI should feel snappy and fluid, with easy use across a variety of devices and screen sizes.
- **Logical Inconsistencies Blocking** - InfoVulcan must prevent users from creating tickets with logical inconsistencies.
    - Closing a ticket before it is opened.
    - Closing or otherwise resolving a ticket without setting a resolution code.
    - Reopening a ticket that is not closed.
- **Modern UI** - InfoVulcan must have a clean, intuitive, and responsive user interface. Dark mode is a must. One technician used ServiceNow as an example of a modern UI that they liked.
- **Policy As Code** - InfoVulcan must enforce ticketing policies through code to ensure consistency and compliance.
- **Robust Reporting and Analytics** - InfoVulcan must include comprehensive reporting and analytics capabilities, enabling users to generate insights from ticket data through customizable reports and dashboards.
- **Scalable** - InfoVulcan must handle large volumes of tickets and users without performance degradation.

Integration with External Systems:
    - Equipment models should be linked to internal KB articles for easy reference.
    - ISP names should be linked to internal KB articles for easy reference.
    - Tracking site department action flags need to be locked to prevent 2 users from working the same TSR/VR/PMAR at the same time.
