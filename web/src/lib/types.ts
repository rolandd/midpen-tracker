export interface User {
    athlete_id: number;
    firstname: string;
    lastname: string;
    profile_picture: string | null;
}

export interface PreserveActivity {
    id: number;
    date: string;
    sport_type: string;
    name: string;
}

export interface PreserveSummary {
    name: string;
    count: number;
    activities: PreserveActivity[];
}

export interface PreserveStatsResponse {
    preserves: PreserveSummary[];
    preserves_by_year: Record<string, Record<string, number>>;
    total_preserves_visited: number;
    total_preserves: number;
    pending_activities: number;
    available_years: string[];
}

export interface ActivitySummary {
    id: number;
    name: string;
    sport_type: string;
    start_date: string;
    preserves: string[];
}

export interface ActivitiesResponse {
    activities: ActivitySummary[];
    total: number;
    page: number;
    per_page: number;
}
