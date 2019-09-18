import React from 'react';

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
        fetch('/api/tour')
            .then(res => res.json())
            .then(
                (result) => {
                    this.setState({
                        isLoaded: true,
                        tours: result
                    });
                },
                // Note: it's important to handle errors here
                // instead of a catch() block so that we don't swallow
                // exceptions from actual bugs in components.
                (error) => {
                    this.setState({
                        isLoaded: true,
                        error
                    });
                })
    }
    
    render() {
        const { error, isLoaded, tours } = this.state;
        if (error) {
            return <div>Error: {error.message}</div>;
        } else if (!isLoaded) {
            return <div>Loading...</div>;
        } else {
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