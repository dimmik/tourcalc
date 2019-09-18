import React from 'react';

export default class TourPersons extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            error: null,
            isLoaded: false,
            tour: {}
        };
    }

    componentDidMount() {
        if (this.props.tourid != null) {
            fetch('/api/tour/' + this.props.tourid + '/calculated')
                .then(res => res.json())
                .then(
                    (result) => {
                        this.setState({
                            isLoaded: true,
                            tour: result
                        });
                    },
                    (error) => {
                        this.setState({
                            isLoaded: true,
                            error
                        });
                    })
        }
    }

    render() {
        if (this.props.tourid == null) return (<div>please choose tour</div>);

        const { error, isLoaded, tour } = this.state;
        if (error) {
            return (<h1>error: {error.message}</h1>)
        } else if (!isLoaded) {
            return (<div>Loading...</div>)
        } else {
            return (
                <div>
                    <ol>
                        {
                            tour.persons.map(
                                (person) => <li key={person.guid}>{person.name} r: {person.receivedInCents * 1.0 / 100} s: {person.spentInCents*1.0 / 100}</li>
                            )

                        }
                    </ol>
                </div>
            )
        }
        
    }
};