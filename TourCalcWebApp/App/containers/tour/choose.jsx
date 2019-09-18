import React from 'react';
import FetchHelper from './helpers.jsx'

export default class TourChoose extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            error: null,
            isLoaded: false,
            tours: []
        };
    }

    componentDidMount() {
        FetchHelper.fetchTours(this)
    }
    
    render() {
        const { error, isLoaded, tours } = this.state;
        if (error) {
            return <div>Error: {error.message}</div>;
        } else if (!isLoaded) {
            return <div>Loading...</div>;
        } else {
            // if not chosen - make first chosen
            if (this.props.chosenTour == null) {
                if (tours.length > 0) {
                    this.props.chooseTourAction(tours[0])
                }
            }
            return (
                <ul>
                    {tours.map(tour => (
                        <li key={tour.id}>
                            <i onClick={() => this.props.chooseTourAction(tour)}>
                                {tour.name} -- {tour.id} -- {this.props.chosenTour == null ? "n/a" : this.props.chosenTour.name}
                            </i>
                        </li>
                    ))}
                </ul>
            );
        }
    }
};