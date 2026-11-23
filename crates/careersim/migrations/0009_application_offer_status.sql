ALTER TABLE applications
    ADD COLUMN offer_response_status text NOT NULL DEFAULT 'pending',
    ADD CONSTRAINT applications_offer_response_status CHECK (
        offer_response_status IN ('pending', 'accepted', 'declined')
    );

CREATE INDEX applications_user_offer_idx
    ON applications (user_id, updated_at DESC, id DESC)
    WHERE status = 'offer';
