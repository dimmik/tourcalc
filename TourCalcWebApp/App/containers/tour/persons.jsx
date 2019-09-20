import React from 'react';
import { Redirect } from 'react-router-dom'
import FetchHelper from './helpers.jsx'

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
            FetchHelper.fetchTourCalculated(this, this.props.tourid)
        }
    }

    render() {
        if (this.props.tourid == null) return (<Redirect to='/choose' />);

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